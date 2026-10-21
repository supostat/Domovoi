use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_agent::{AgentContext, ClaudeCode, ClaudeCodeError, FocusContext, PastTurn};
use domovoi_grammar::Tool;
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_inference::CancelFlag;
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, LevelTotals, Recommendations, RuleSet};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use serde_json::Value;

const PROXY: &str = env!("CARGO_BIN_EXE_domovoi");
const MODEL: &str = "sonnet";
const MAX_TURNS: u32 = 6;
const QUESTION: &str = "Сколько места занято?";
const ANSWER: &str = "Занято немного: в основном сборка engine.";
const SESSION: &str = "fake-session-1";
const EXECUTABLE: u32 = 0o755;
const STOP_BUDGET: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(10);
const ANSWERING: &str = r#"
config=""
previous=""
for argument in "$@"; do
  if [ "$previous" = "--mcp-config" ]; then config="$argument"; fi
  previous="$argument"
done
field() { printf '%s' "$config" | plutil -extract "mcpServers.domovoi.$1" raw -o - -; }
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"fake","version":"1"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"summarize","arguments":{}}}' \
  | "$(field command)" "$(field args.0)" "$(field args.1)" > "$OUTBOX/replies"
printf '%s' '{"type":"result","subtype":"success","is_error":false,"result":"ANSWER","session_id":"SESSION"}'
"#;
const FAILING: &str = r#"
printf '%s' '{"type":"result","subtype":"error_during_execution","is_error":true,"result":"usage limit reached","session_id":"SESSION"}'
exit 1
"#;
const HANGING: &str = "exec sleep 30\n";

struct Disk {
    tree: TempTree,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    totals: LevelTotals,
    home: PathBuf,
}

impl Disk {
    fn new() -> Self {
        let tree = TempTree::new().expect("a temp tree is created");
        let root = tree.root().expect("the root opens");
        let engine = root.directory("engine").expect("a directory is created");
        engine.file("Cargo.toml", 64, Fill::Byte(1)).expect("a file is written");
        engine
            .directory("target")
            .expect("a directory is created")
            .file("engine.bin", 90_000, Fill::Noise(1))
            .expect("a file is written");
        let report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
        let index = Index::build(
            &report,
            IndexRoot::for_path(tree.path()).expect("the root kind is known"),
            &Enrichment::default(),
        );
        let rules = RuleSet::for_volume(tree.path(), &ConfigOverrides::default()).expect("the rules are prepared");
        let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
        let totals = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new()).totals;
        let home = index.path(index.root());
        Self {
            tree,
            index,
            rules,
            denylist,
            totals,
            home,
        }
    }

    fn context<'a>(&'a self, history: &'a [PastTurn]) -> AgentContext<'a> {
        AgentContext {
            index: &self.index,
            rules: &self.rules,
            denylist: &self.denylist,
            totals: self.totals,
            home: &self.home,
            history,
            focus: FocusContext::none(),
        }
    }
}

struct FakeClaude {
    directory: TempTree,
    program: PathBuf,
}

impl FakeClaude {
    fn behaving(body: &str) -> Self {
        let directory = TempTree::new().expect("a temp tree is created");
        let outbox = directory.path().to_string_lossy().into_owned();
        let script = format!(
            "#!/bin/sh\nOUTBOX='{outbox}'\nprintf '%s\\0' \"$@\" > \"$OUTBOX/arguments\"\ncat > \"$OUTBOX/prompt\"\necho $$ > \"$OUTBOX/pid\"\n{}",
            body.replace("ANSWER", ANSWER).replace("SESSION", SESSION)
        );
        let program = directory.path().join("claude");
        fs::write(&program, script).expect("the fake claude is written");
        fs::set_permissions(&program, fs::Permissions::from_mode(EXECUTABLE)).expect("the fake claude is executable");
        fs::create_dir(directory.path().join("workspace")).expect("the workspace is created");
        Self { directory, program }
    }

    fn claude_code(&self) -> ClaudeCode {
        ClaudeCode {
            program: self.program.clone(),
            proxy: PathBuf::from(PROXY),
            workspace: self.directory.path().join("workspace"),
            model: MODEL.to_owned(),
            max_turns: MAX_TURNS,
            keep_sessions: true,
        }
    }

    fn written(&self, name: &str) -> String {
        fs::read_to_string(self.directory.path().join(name)).unwrap_or_else(|_| panic!("the fake claude wrote {name}"))
    }

    fn arguments(&self) -> Vec<String> {
        self.written("arguments")
            .split_terminator('\0')
            .map(str::to_owned)
            .collect()
    }
}

fn value_after<'a>(arguments: &'a [String], flag: &str) -> &'a str {
    let position = arguments
        .iter()
        .position(|argument| argument == flag)
        .unwrap_or_else(|| panic!("{flag} is passed: {arguments:?}"));
    &arguments[position + 1]
}

fn exists(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stderr(Stdio::null())
        .status()
        .expect("kill runs")
        .success()
}

#[test]
fn the_fake_claude_gets_every_flag_and_reaches_the_tools_through_the_real_proxy() {
    let disk = Disk::new();
    let claude = FakeClaude::behaving(ANSWERING);
    let mut observed = Vec::new();
    let answer = claude
        .claude_code()
        .ask(
            &disk.context(&[]),
            QUESTION,
            None,
            &CancelFlag::default(),
            &mut |step| observed.push(step.tool),
        )
        .expect("the fake claude answers");

    assert_eq!(answer.answer.text, ANSWER);
    assert_eq!(answer.session_id, SESSION);
    assert_eq!(observed, [Tool::Summarize]);
    assert_eq!(answer.answer.steps.len(), 1);
    let replies: Vec<Value> = claude
        .written("replies")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a reply is JSON"))
        .collect();
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "domovoi");
    let summary = replies[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("the summary is text");
    assert!(summary.contains("used_bytes"), "{summary}");

    let arguments = claude.arguments();
    for flag in ["-p", "--strict-mcp-config"] {
        assert!(
            arguments.iter().any(|argument| argument == flag),
            "{flag}: {arguments:?}"
        );
    }
    assert_eq!(value_after(&arguments, "--output-format"), "json");
    assert_eq!(value_after(&arguments, "--setting-sources"), "");
    assert_eq!(value_after(&arguments, "--tools"), "");
    assert_eq!(value_after(&arguments, "--allowedTools"), "mcp__domovoi__*");
    assert_eq!(value_after(&arguments, "--model"), MODEL);
    assert_eq!(value_after(&arguments, "--max-turns"), MAX_TURNS.to_string());
    let instructions = value_after(&arguments, "--append-system-prompt");
    assert!(
        instructions.contains("Домовой") && instructions.contains("note_hypothesis"),
        "{instructions}"
    );
    let config: Value = serde_json::from_str(value_after(&arguments, "--mcp-config")).expect("the MCP config is JSON");
    assert_eq!(config["mcpServers"]["domovoi"]["command"], PROXY);
    assert_eq!(config["mcpServers"]["domovoi"]["args"][0], "--mcp-proxy");
    for absent in ["--resume", "--no-session-persistence"] {
        assert!(
            !arguments.iter().any(|argument| argument == absent),
            "{absent}: {arguments:?}"
        );
    }
    let prompt = claude.written("prompt");
    assert!(
        prompt.contains("Сводка индекса") && prompt.contains(QUESTION),
        "{prompt}"
    );
}

#[test]
fn a_known_session_resumes_and_a_fresh_one_carries_the_earlier_turns_as_text() {
    let disk = Disk::new();
    let claude = FakeClaude::behaving(ANSWERING);
    let history = [PastTurn {
        question: "Что в engine?".to_owned(),
        answer: "Там сборка на 90 КБ.".to_owned(),
        paths: vec![disk.tree.path().join("engine").to_string_lossy().into_owned()],
    }];
    let ask = |session: Option<&str>| {
        claude
            .claude_code()
            .ask(
                &disk.context(&history),
                QUESTION,
                session,
                &CancelFlag::default(),
                &mut |_| {},
            )
            .expect("the fake claude answers");
    };

    ask(Some(SESSION));
    assert_eq!(value_after(&claude.arguments(), "--resume"), SESSION);
    let resumed = claude.written("prompt");
    assert!(!resumed.contains("Что в engine?"), "{resumed}");

    ask(None);
    assert!(!claude.arguments().iter().any(|argument| argument == "--resume"));
    let fresh = claude.written("prompt");
    let earlier = fresh
        .find("Что в engine?")
        .expect("the earlier question is in the prompt");
    assert!(fresh.contains("Там сборка на 90 КБ."), "{fresh}");
    assert!(earlier < fresh.find(QUESTION).expect("the question is in the prompt"));
}

#[test]
fn an_error_reply_is_a_failure() {
    let disk = Disk::new();
    let claude = FakeClaude::behaving(FAILING);
    let outcome = claude
        .claude_code()
        .ask(&disk.context(&[]), QUESTION, None, &CancelFlag::default(), &mut |_| {});
    match outcome {
        Err(ClaudeCodeError::Failed(reason)) => assert!(reason.contains("usage limit reached"), "{reason}"),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn cancel_stops_the_claude_process() {
    let disk = Disk::new();
    let claude = FakeClaude::behaving(HANGING);
    let cancel = CancelFlag::default();
    let pid_file = claude.directory.path().join("pid");
    let canceller = {
        let cancel = cancel.clone();
        let pid_file = pid_file.clone();
        thread::spawn(move || {
            while !pid_file.exists() {
                thread::sleep(POLL);
            }
            cancel.raise();
        })
    };
    let started = Instant::now();
    let outcome = claude
        .claude_code()
        .ask(&disk.context(&[]), QUESTION, None, &cancel, &mut |_| {});
    canceller.join().expect("the canceller finishes");

    assert!(matches!(outcome, Err(ClaudeCodeError::Cancelled)), "{outcome:?}");
    assert!(started.elapsed() < STOP_BUDGET, "{:?}", started.elapsed());
    let pid = fs::read_to_string(&pid_file).expect("the pid is written");
    assert!(!exists(pid.trim()), "claude {pid} still runs");
}
