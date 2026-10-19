use std::fs::{self, File};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_agent::AgentLimits;
use domovoi_app::{ChatEntry, ChatModel, InferenceClient, ServiceAddress, Session, SessionPaths, strings};
use domovoi_downloader::{Catalog, CatalogEntry};
use domovoi_inference::{
    AnonymousService, CancelFlag, FakeBackend, InferenceBackend, InferenceService, PRESSURE_IDLE_GRACE, PressureLevel,
};
use domovoi_inference_proto::Timeouts;
use domovoi_scanner::CancelToken;
use domovoi_testkit::{TempTree, test_timeout};
use serde_json::{Value, json};

const QUESTION: &str = "Что из разработки можно убрать?";
const ANSWER: &str = "Результаты сборки Rust пересоздаются, их можно убрать.";
const BUILD_OUTPUT: usize = 2 * 1024 * 1024;
const WARN_AFTER: Duration = Duration::from_millis(150);
const STEP_DELAY_MILLISECONDS: u64 = 400;
const GRACE_MARGIN: Duration = Duration::from_millis(500);
const LOG_POLL: Duration = Duration::from_millis(250);
const RELOADED: &str = "the inference service unloaded the model; the weights are loaded again";
const CLIENT_REFUSAL: &str = "the inference service answered OutOfMemory: memory pressure is critical";
const SERVICE_REFUSAL: &str = "a model load is refused: memory pressure is critical";

fn say(response: &Value) -> String {
    format!("token {response}")
}

fn tool_call() -> String {
    say(&json!({"kind": "tool_call", "name": "query_index", "arguments": {"level": "safe", "limit": 10}}))
}

fn final_answer() -> String {
    say(&json!({"kind": "final", "answer": ANSWER, "hypotheses": []}))
}

fn populate(root: &Path) {
    let project = root.join("Projects/engine");
    fs::create_dir_all(project.join("target/debug")).expect("the project tree is created");
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"engine\"\n").expect("the manifest is written");
    fs::write(project.join("target/debug/engine"), vec![7_u8; BUILD_OUTPUT]).expect("the build output is written");
}

fn catalog_entry() -> CatalogEntry {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/test/catalog.json");
    Catalog::from_path(&path).expect("the test catalog parses").entries[0].clone()
}

struct Assistant {
    session: Session,
    entry: CatalogEntry,
    client: InferenceClient,
    weights: PathBuf,
    chat: ChatModel,
}

impl Assistant {
    fn weights(&self) -> OwnedFd {
        File::open(&self.weights).expect("the weights open").into()
    }

    fn load(&mut self) {
        let weights = self.weights();
        self.client
            .load(
                weights,
                self.entry.class,
                self.entry.context_length,
                &CancelFlag::default(),
            )
            .expect("the weights load");
    }

    fn ask(&mut self) {
        if !self.client.is_loaded() {
            self.load();
        }
        self.chat.ask(QUESTION);
        let chat = &mut self.chat;
        let answer = self.session.ask(
            &mut self.client,
            &self.entry,
            AgentLimits::for_context_length(self.entry.context_length),
            QUESTION,
            &CancelFlag::default(),
            &mut |step| chat.record_step(step),
        );
        match answer {
            Ok(answer) => self.chat.record_answer(&answer),
            Err(error) => self.chat.record_failure(&error),
        }
    }

    fn last_entry(&self) -> &ChatEntry {
        self.chat.entries.last().expect("the chat has entries")
    }
}

fn answered(entry: &ChatEntry) -> bool {
    matches!(entry, ChatEntry::Answer { text, .. } if text == ANSWER)
}

fn logged() -> String {
    let predicate = format!(
        "processID == {} AND subsystem BEGINSWITH \"com.domovoi\"",
        process::id()
    );
    let output = Command::new("/usr/bin/log")
        .args(["show", "--last", "1m", "--style", "compact", "--predicate", &predicate])
        .output()
        .expect("log show runs");
    assert!(output.status.success(), "log show failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn report_during(service: &InferenceService, level: PressureLevel, action: impl FnOnce()) {
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(WARN_AFTER);
            service.report_pressure(level);
        });
        action();
    });
}

#[test]
fn a_question_under_memory_pressure_gets_an_answer_or_an_honest_out_of_memory() {
    let volume = TempTree::new().expect("a temp tree is created");
    populate(volume.path());
    let state = TempTree::new().expect("a temp tree is created");
    let mut session = Session::open(volume.path(), &SessionPaths::in_directory(state.path()), "0.0.0-test")
        .expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the tree scans");

    let delayed_call = format!("delay {STEP_DELAY_MILLISECONDS}\n{}", tool_call());
    let service = AnonymousService::start(
        Box::new(FakeBackend::replying([
            delayed_call,
            final_answer(),
            tool_call(),
            final_answer(),
            tool_call(),
            final_answer(),
        ])),
        None,
    );
    let pressure = service.service();
    let weights = state.path().join("weights.gguf");
    fs::write(&weights, b"GGUF wire").expect("the weights are written");
    let timeouts = Timeouts::from_environment().expect("the timeouts parse");
    let mut assistant = Assistant {
        session,
        entry: catalog_entry(),
        client: InferenceClient::connect(ServiceAddress::Endpoint(service.endpoint()), timeouts)
            .expect("the client connects"),
        weights,
        chat: ChatModel::default(),
    };

    report_during(pressure, PressureLevel::Warning, || assistant.ask());
    assert!(
        answered(assistant.last_entry()),
        "a warning in the middle of an answer lets it finish: {:?}",
        assistant.last_entry()
    );

    thread::sleep(PRESSURE_IDLE_GRACE + GRACE_MARGIN);
    assistant.ask();
    assert!(
        answered(assistant.last_entry()),
        "after the idle unload the next question reloads the model by itself: {:?}",
        assistant.last_entry()
    );

    pressure.report_pressure(PressureLevel::Critical);
    assistant.ask();
    assert_eq!(
        assistant.last_entry(),
        &ChatEntry::Failure {
            text: strings::ANSWER_LOW_MEMORY.to_owned()
        },
        "critical pressure reads as low memory, never as a stop"
    );

    pressure.report_pressure(PressureLevel::Warning);
    assistant.ask();
    assert!(
        answered(assistant.last_entry()),
        "once the pressure eases below critical the question is answered: {:?}",
        assistant.last_entry()
    );

    let deadline = Instant::now() + test_timeout();
    loop {
        let records = logged();
        if [RELOADED, CLIENT_REFUSAL, SERVICE_REFUSAL]
            .iter()
            .all(|line| records.contains(line))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the unified log misses the reload or the refusal:\n{records}"
        );
        thread::sleep(LOG_POLL);
    }
}
