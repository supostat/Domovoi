#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use domovoi_agent::ClaudeCode;
use domovoi_testkit::TempTree;

pub(crate) const PROXY: &str = env!("CARGO_BIN_EXE_domovoi");
pub(crate) const MODEL: &str = "sonnet";
pub(crate) const MAX_TURNS: u32 = 6;
pub(crate) const ANSWER: &str = "Занято немного: в основном сборка engine.";
pub(crate) const SESSION: &str = "fake-session-1";
const EXECUTABLE: u32 = 0o755;
pub(crate) const ANSWERING: &str = r#"
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
pub(crate) const FAILING: &str = r#"
printf '%s' '{"type":"result","subtype":"error_during_execution","is_error":true,"result":"usage limit reached","session_id":"SESSION"}'
exit 1
"#;
pub(crate) const HANGING: &str = "exec sleep 30\n";

pub(crate) struct FakeClaude {
    directory: TempTree,
    program: PathBuf,
}

impl FakeClaude {
    pub(crate) fn behaving(body: &str) -> Self {
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

    pub(crate) fn claude_code(&self) -> ClaudeCode {
        ClaudeCode {
            program: self.program.clone(),
            proxy: PathBuf::from(PROXY),
            workspace: self.directory.path().join("workspace"),
            model: MODEL.to_owned(),
            max_turns: MAX_TURNS,
            keep_sessions: true,
        }
    }

    pub(crate) fn written(&self, name: &str) -> String {
        fs::read_to_string(self.directory.path().join(name)).unwrap_or_else(|_| panic!("the fake claude wrote {name}"))
    }

    pub(crate) fn arguments(&self) -> Vec<String> {
        self.written("arguments")
            .split_terminator('\0')
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn pid_file(&self) -> PathBuf {
        self.directory.path().join("pid")
    }
}

pub(crate) fn value_after<'a>(arguments: &'a [String], flag: &str) -> &'a str {
    let position = arguments
        .iter()
        .position(|argument| argument == flag)
        .unwrap_or_else(|| panic!("{flag} is passed: {arguments:?}"));
    &arguments[position + 1]
}

pub(crate) fn is_running(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stderr(Stdio::null())
        .status()
        .expect("kill runs")
        .success()
}
