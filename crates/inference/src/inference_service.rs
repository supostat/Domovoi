use std::ffi::CStr;
use std::os::fd::OwnedFd;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use domovoi_inference_proto::{DecodeError, ErrorCode, GenerateRequest, Message, ModelClass, StopReason};
use domovoi_log_sys::SystemLog;
use domovoi_xpc_sys::XpcDictionary;

use crate::backend_error::BackendError;
use crate::cancel_flag::CancelFlag;
use crate::inference_backend::InferenceBackend;
use crate::message_sink::MessageSink;
use crate::pressure_level::PressureLevel;

const WORKER_NAME: &str = "domovoi-generation";
const LOG_SUBSYSTEM: &CStr = c"com.domovoi.inference";
const LOG_CATEGORY: &CStr = c"memory";
const CRITICAL_PRESSURE: &str = "memory pressure is critical";

pub const PRESSURE_IDLE_GRACE: Duration = Duration::from_secs(8);

#[derive(Debug)]
pub struct InferenceService {
    commands: Sender<Command>,
    cancel: CancelFlag,
    pressure_interrupted: Arc<AtomicBool>,
    log: SystemLog,
}

enum Command {
    Load {
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
        reply: Reply,
    },
    Unload {
        reply: Reply,
    },
    Generate {
        request: GenerateRequest,
        reply: Reply,
    },
    CountTokens {
        text: String,
        reply: Reply,
    },
    Pressure(PressureLevel),
}

struct Reply {
    request: XpcDictionary,
    sink: Arc<dyn MessageSink>,
}

impl Reply {
    fn stream(&self, message: &Message) {
        match message.encode() {
            Ok(encoded) => self.sink.deliver(&encoded),
            Err(error) => eprintln!("domovoi-inference: a streamed message did not encode: {error}"),
        }
    }

    fn send(self, message: &Message) {
        let mut reply = self.request.reply().unwrap_or_default();
        match message.encode_into(&mut reply) {
            Ok(()) => self.sink.deliver(&reply),
            Err(error) => eprintln!("domovoi-inference: a reply did not encode: {error}"),
        }
    }
}

impl InferenceService {
    pub fn start(backend: Box<dyn InferenceBackend>, idle_unload: Option<Duration>) -> Self {
        let (commands, receiver) = mpsc::channel();
        let cancel = CancelFlag::default();
        let pressure_interrupted = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            backend,
            cancel: cancel.clone(),
            pressure_interrupted: Arc::clone(&pressure_interrupted),
            idle_unload,
            refusing_loads: false,
            unload_when_idle: false,
            log: SystemLog::new(LOG_SUBSYSTEM, LOG_CATEGORY),
        };
        thread::Builder::new()
            .name(WORKER_NAME.to_owned())
            .spawn(move || {
                unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0) };
                worker.run(&receiver);
            })
            .expect("the generation worker starts");
        Self {
            commands,
            cancel,
            pressure_interrupted,
            log: SystemLog::new(LOG_SUBSYSTEM, LOG_CATEGORY),
        }
    }

    pub fn handle(&self, request: XpcDictionary, sink: Arc<dyn MessageSink>) {
        let reply = Reply { request, sink };
        let message = match Message::decode(&reply.request) {
            Ok(message) => message,
            Err(error @ (DecodeError::MissingVersion | DecodeError::UnsupportedVersion(_))) => {
                reply.send(&BackendError::new(ErrorCode::ProtocolMismatch, error.to_string()).into_message());
                return;
            }
            Err(error) => {
                reply.send(&BackendError::new(ErrorCode::Internal, error.to_string()).into_message());
                return;
            }
        };
        match message {
            Message::Handshake { .. } => reply.send(&Message::Handshake {
                pid: libc::pid_t::try_from(process::id()).expect("a process id fits in pid_t"),
            }),
            Message::Cancel => {
                self.cancel.raise();
                reply.send(&Message::Ready);
            }
            Message::Load {
                weights,
                class,
                context_length,
            } => self.enqueue(Command::Load {
                weights,
                class,
                context_length,
                reply,
            }),
            Message::Unload => self.enqueue(Command::Unload { reply }),
            Message::Generate(request) => self.enqueue(Command::Generate { request, reply }),
            Message::CountTokens { text } => self.enqueue(Command::CountTokens { text, reply }),
            other => reply.send(
                &BackendError::new(
                    ErrorCode::Internal,
                    format!("the service does not accept {}", other.kind().to_string_lossy()),
                )
                .into_message(),
            ),
        }
    }

    pub fn report_pressure(&self, level: PressureLevel) {
        self.log.notice(&format!("memory pressure level: {level:?}"));
        if level == PressureLevel::Critical {
            self.pressure_interrupted.store(true, Ordering::SeqCst);
            self.cancel.raise();
        }
        self.enqueue(Command::Pressure(level));
    }

    fn enqueue(&self, command: Command) {
        self.commands
            .send(command)
            .expect("the generation worker lives as long as the service");
    }
}

struct Worker {
    backend: Box<dyn InferenceBackend>,
    cancel: CancelFlag,
    pressure_interrupted: Arc<AtomicBool>,
    idle_unload: Option<Duration>,
    refusing_loads: bool,
    unload_when_idle: bool,
    log: SystemLog,
}

impl Worker {
    fn run(mut self, receiver: &Receiver<Command>) {
        loop {
            let command = match self.idle_limit() {
                Some(limit) => match receiver.recv_timeout(limit) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => {
                        self.unload_idle();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                },
                None => match receiver.recv() {
                    Ok(command) => command,
                    Err(_) => return,
                },
            };
            self.execute(command);
        }
    }

    fn idle_limit(&self) -> Option<Duration> {
        if !self.backend.is_loaded() {
            return None;
        }
        [self.idle_unload, self.unload_when_idle.then_some(PRESSURE_IDLE_GRACE)]
            .into_iter()
            .flatten()
            .min()
    }

    fn unload_idle(&mut self) {
        if self.unload_when_idle {
            self.log.notice("the idle model is unloaded under memory pressure");
        }
        self.backend.unload();
    }

    fn begin_work(&self) {
        self.pressure_interrupted.store(false, Ordering::SeqCst);
        self.cancel.lower();
    }

    fn interrupted_by_pressure(&self) -> bool {
        self.pressure_interrupted.load(Ordering::SeqCst)
    }

    fn refuse(&self, reply: Reply, what: &str) {
        self.log.error(&format!("{what}: {CRITICAL_PRESSURE}"));
        reply.send(&BackendError::new(ErrorCode::OutOfMemory, CRITICAL_PRESSURE).into_message());
    }

    fn execute(&mut self, command: Command) {
        match command {
            Command::Load {
                weights,
                class,
                context_length,
                reply,
            } => {
                if self.refusing_loads {
                    return self.refuse(reply, "a model load is refused");
                }
                self.begin_work();
                match self.backend.load(weights, class, context_length, &self.cancel) {
                    Ok(()) => reply.send(&Message::Ready),
                    Err(error) if error.code == ErrorCode::LoadCancelled && self.interrupted_by_pressure() => {
                        self.refuse(reply, "a model load is interrupted");
                    }
                    Err(error) => reply.send(&error.into_message()),
                }
            }
            Command::Unload { reply } => {
                self.backend.unload();
                reply.send(&Message::Ready);
            }
            Command::Generate { request, reply } => {
                self.begin_work();
                let outcome = self.backend.generate(&request, &self.cancel, &mut |text| {
                    reply.stream(&Message::Token { text: text.to_owned() });
                });
                match outcome {
                    Ok(stats) if stats.stop == StopReason::Cancelled && self.interrupted_by_pressure() => {
                        self.refuse(reply, "a generation is interrupted");
                    }
                    Ok(stats) => reply.send(&Message::Done(stats)),
                    Err(error) => reply.send(&error.into_message()),
                }
            }
            Command::CountTokens { text, reply } => match self.backend.count_tokens(&text) {
                Ok(count) => reply.send(&Message::TokenCount { count }),
                Err(error) => reply.send(&error.into_message()),
            },
            Command::Pressure(PressureLevel::Normal) => {
                self.refusing_loads = false;
                self.unload_when_idle = false;
            }
            Command::Pressure(PressureLevel::Warning) => {
                self.refusing_loads = false;
                self.unload_when_idle = true;
            }
            Command::Pressure(PressureLevel::Critical) => {
                self.backend.unload();
                self.refusing_loads = true;
            }
        }
    }
}
