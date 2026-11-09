use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, ExitCode, Stdio};

use domovoi_app::{Session, SessionPaths, TrayModel};
use domovoi_index::{Index, NodeId};
use domovoi_rules::DenyReason;
use domovoi_scanner::CancelToken;
use domovoi_testkit::TempTree;

const LISTEN: &str = "--listen";
const BUNDLE: &str = "Tools/Listener.app";
const EXECUTABLE: &str = "Contents/MacOS/listener";
const GREETING: &str = "ready\n";

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.try_wait().ok();
    }
}

fn listen() -> ExitCode {
    let mut output = io::stdout();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else {
            return ExitCode::FAILURE;
        };
        if writeln!(output, "{line}").and_then(|()| output.flush()).is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn start(executable: &Path) -> Running {
    let mut running = Running(
        Command::new(executable)
            .arg(LISTEN)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the program starts"),
    );
    running
        .0
        .stdin
        .as_mut()
        .expect("the program reads its input")
        .write_all(GREETING.as_bytes())
        .expect("the program is greeted");
    let mut answer = String::new();
    BufReader::new(running.0.stdout.as_mut().expect("the program writes its output"))
        .read_line(&mut answer)
        .expect("the program answers");
    assert_eq!(answer, GREETING, "the program runs and answers");
    running
}

fn node_at(index: &Index, relative: &str) -> NodeId {
    Path::new(relative).iter().fold(index.root(), |node, name| {
        index
            .child_named(node, name)
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    })
}

fn a_program_started_after_the_session_opened_is_not_deleted() -> Result<(), String> {
    let tree = TempTree::new().expect("a temp tree is created");
    let executable = tree.path().join(BUNDLE).join(EXECUTABLE);
    fs::create_dir_all(executable.parent().expect("an executable sits in a folder")).expect("the bundle is created");
    fs::copy(env::current_exe().expect("the check knows its program"), &executable)
        .expect("the program is copied into the bundle");
    let state = TempTree::new().expect("a state folder is created");
    let mut session =
        Session::open(tree.path(), &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the tree scans");
    let _running = start(&executable);
    let index = session.index().expect("the tree is indexed");
    session.add_to_tray(node_at(index, BUNDLE), None);
    let index = session.index().expect("the tree is indexed");
    let sizes = TrayModel::estimated_sizes(index, session.tray());

    let (subjects, refusals) = session
        .take_tray_for_delete(&sizes)
        .expect("running programs are checked");

    let refused_as_running = matches!(
        refusals.as_slice(),
        [refusal] if matches!(refusal.reason, DenyReason::RunningApplication(_))
    );
    if subjects.is_empty() && refused_as_running && executable.exists() {
        Ok(())
    } else {
        Err(format!(
            "a program started after the session opened would be deleted: {subjects:?} {refusals:?}"
        ))
    }
}

fn main() -> ExitCode {
    if env::args().nth(1).as_deref() == Some(LISTEN) {
        return listen();
    }
    match a_program_started_after_the_session_opened_is_not_deleted() {
        Ok(()) => {
            println!("ok   a program started after the session opened is not deleted");
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("FAIL {failure}");
            ExitCode::FAILURE
        }
    }
}
