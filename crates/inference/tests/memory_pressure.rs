use std::fs::{self, File};
use std::os::fd::OwnedFd;
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_inference::{
    AnonymousService, BackendError, CancelFlag, FakeBackend, InferenceBackend, PRESSURE_IDLE_GRACE, PressureLevel,
};
use domovoi_inference_proto::{ErrorCode, GenerateRequest, GenerationStats, Message, ModelClass, Sampling, StopReason};
use domovoi_testkit::{TempTree, test_timeout};
use domovoi_xpc_sys::{XpcConnection, XpcEvent};

const CONTEXT_LENGTH: u32 = 512;
const INTERRUPT_AFTER: Duration = Duration::from_millis(100);
const GRACE_MARGIN: Duration = Duration::from_millis(500);
const LOG_POLL: Duration = Duration::from_millis(250);
const SERVICE_SUBSYSTEM: &str = "com.domovoi.inference";

struct LoadUntilCancelled;

impl InferenceBackend for LoadUntilCancelled {
    fn load(
        &mut self,
        _weights: OwnedFd,
        _class: ModelClass,
        _context_length: u32,
        cancel: &CancelFlag,
    ) -> Result<(), BackendError> {
        let deadline = Instant::now() + test_timeout();
        while !cancel.is_raised() {
            assert!(Instant::now() < deadline, "the load was never cancelled");
            thread::sleep(Duration::from_millis(5));
        }
        Err(BackendError::new(ErrorCode::LoadCancelled, "loading was cancelled"))
    }

    fn unload(&mut self) {}

    fn is_loaded(&self) -> bool {
        false
    }

    fn generate(
        &mut self,
        _request: &GenerateRequest,
        _cancel: &CancelFlag,
        _on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        Err(BackendError::new(ErrorCode::ModelNotLoaded, "no model is loaded"))
    }

    fn count_tokens(&mut self, _text: &str) -> Result<u32, BackendError> {
        Err(BackendError::new(ErrorCode::ModelNotLoaded, "no model is loaded"))
    }
}

struct Client {
    service: AnonymousService,
    connection: XpcConnection,
    workspace: TempTree,
}

impl Client {
    fn of(backend: Box<dyn InferenceBackend>) -> Self {
        let service = AnonymousService::start(backend, None);
        let connection = service.endpoint().connect();
        connection.set_event_handler(|_| {});
        connection.activate();
        Self {
            service,
            connection,
            workspace: TempTree::new().expect("a temp tree is created"),
        }
    }

    fn request(&self, message: &Message) -> Message {
        match self
            .connection
            .send_with_reply_sync(&message.encode().expect("the request encodes"))
        {
            XpcEvent::Message(reply) => Message::decode(&reply).expect("the reply decodes"),
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    fn load(&self) -> Message {
        let weights = self.workspace.path().join("weights.gguf");
        fs::write(&weights, b"GGUF").expect("the weights are written");
        self.request(&Message::Load {
            weights: File::open(&weights).expect("the weights open").into(),
            class: ModelClass::Compact,
            context_length: CONTEXT_LENGTH,
        })
    }

    fn generate(&self, script: &[&str]) -> Message {
        self.request(&Message::Generate(GenerateRequest {
            prompt: script.join("\n"),
            grammar: None,
            sampling: Sampling {
                temperature: 0.0,
                top_k: 0,
                top_p: 1.0,
                seed: 0,
            },
            max_tokens: 8,
        }))
    }

    fn report_later(&self, level: PressureLevel, action: impl FnOnce() -> Message) -> Message {
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(INTERRUPT_AFTER);
                self.service.service().report_pressure(level);
            });
            action()
        })
    }

    fn cancel_later(&self, action: impl FnOnce() -> Message) -> Message {
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(INTERRUPT_AFTER);
                assert!(matches!(self.request(&Message::Cancel), Message::Ready));
            });
            action()
        })
    }
}

fn finished(reply: &Message) -> Option<&GenerationStats> {
    match reply {
        Message::Done(stats) => Some(stats),
        _ => None,
    }
}

fn failed_with(reply: &Message, expected: ErrorCode) -> bool {
    matches!(reply, Message::Error { code, .. } if *code == expected)
}

fn service_log_shows(text: &str) -> bool {
    let predicate = format!(
        "processID == {} AND subsystem == \"{SERVICE_SUBSYSTEM}\" AND eventMessage CONTAINS \"{text}\"",
        process::id()
    );
    let output = Command::new("/usr/bin/log")
        .args(["show", "--last", "1m", "--style", "compact", "--predicate", &predicate])
        .output()
        .expect("log show runs");
    assert!(output.status.success(), "log show failed");
    String::from_utf8_lossy(&output.stdout).contains(text)
}

#[test]
fn a_warning_lets_the_answer_finish_and_unloads_the_model_only_after_the_idle_grace() {
    let client = Client::of(Box::new(FakeBackend::default()));
    assert!(matches!(client.load(), Message::Ready));

    let reply = client.report_later(PressureLevel::Warning, || {
        client.generate(&["token до", "delay 400", "token после"])
    });
    let stats = finished(&reply).unwrap_or_else(|| panic!("the generation finishes: {reply:?}"));
    assert_eq!(stats.stop, StopReason::EndOfGeneration);
    assert_eq!(stats.generated_tokens, 2);

    let within_grace = client.generate(&["token ещё"]);
    assert!(
        finished(&within_grace).is_some(),
        "the next step of the answer runs without a reload: {within_grace:?}"
    );

    thread::sleep(PRESSURE_IDLE_GRACE + GRACE_MARGIN);
    let after_grace = client.generate(&["token поздно"]);
    assert!(
        failed_with(&after_grace, ErrorCode::ModelNotLoaded),
        "the idle model is unloaded after the grace: {after_grace:?}"
    );
}

#[test]
fn critical_pressure_interrupts_a_generation_as_out_of_memory_and_refuses_loads_until_it_eases() {
    let client = Client::of(Box::new(FakeBackend::default()));
    assert!(matches!(client.load(), Message::Ready));

    let interrupted = client.report_later(PressureLevel::Critical, || {
        client.generate(&["token до", "delay 5000", "token после"])
    });
    assert!(failed_with(&interrupted, ErrorCode::OutOfMemory), "{interrupted:?}");

    let refused = client.load();
    assert!(failed_with(&refused, ErrorCode::OutOfMemory), "{refused:?}");

    client.service.service().report_pressure(PressureLevel::Warning);
    let allowed = client.load();
    assert!(
        matches!(allowed, Message::Ready),
        "a warning lifts the refusal: {allowed:?}"
    );

    let deadline = Instant::now() + test_timeout();
    while !service_log_shows("a model load is refused: memory pressure is critical") {
        assert!(Instant::now() < deadline, "the refusal never reached the unified log");
        thread::sleep(LOG_POLL);
    }
}

#[test]
fn critical_pressure_interrupts_a_load_as_out_of_memory() {
    let client = Client::of(Box::new(LoadUntilCancelled));
    let reply = client.report_later(PressureLevel::Critical, || client.load());
    assert!(failed_with(&reply, ErrorCode::OutOfMemory), "{reply:?}");
}

#[test]
fn the_users_cancel_stays_a_cancellation_for_loads_and_generations() {
    let loading = Client::of(Box::new(LoadUntilCancelled));
    let load = loading.cancel_later(|| loading.load());
    assert!(failed_with(&load, ErrorCode::LoadCancelled), "{load:?}");

    let generating = Client::of(Box::new(FakeBackend::default()));
    assert!(matches!(generating.load(), Message::Ready));
    let reply = generating.cancel_later(|| generating.generate(&["token до", "delay 5000", "token после"]));
    let stats = finished(&reply).unwrap_or_else(|| panic!("a cancelled generation is done: {reply:?}"));
    assert_eq!(stats.stop, StopReason::Cancelled);
}
