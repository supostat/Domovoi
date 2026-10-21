mod fake_claude;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use domovoi_agent::{AgentContext, ClaudeCodeError, FocusContext, PastTurn};
use domovoi_grammar::Tool;
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_inference::CancelFlag;
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, LevelTotals, Recommendations, RuleSet};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use fake_claude::{
    ANSWER, ANSWERING, FAILING, FakeClaude, HANGING, MAX_TURNS, MODEL, PROXY, SESSION, is_running, value_after,
};
use serde_json::Value;

const QUESTION: &str = "Сколько места занято?";
const STOP_BUDGET: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(10);

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
    let pid_file = claude.pid_file();
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
    assert!(!is_running(pid.trim()), "claude {pid} still runs");
}
