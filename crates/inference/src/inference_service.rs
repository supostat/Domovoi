use std::os::fd::OwnedFd;
use std::process;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use domovoi_inference_proto::{DecodeError, ErrorCode, GenerateRequest, Message, ModelClass};
use domovoi_xpc_sys::XpcDictionary;

use crate::backend_error::BackendError;
use crate::cancel_flag::CancelFlag;
use crate::inference_backend::InferenceBackend;
use crate::message_sink::MessageSink;
use crate::pressure_level::PressureLevel;

const WORKER_NAME: &str = "domovoi-generation";

#[derive(Debug)]
pub struct InferenceService {
    commands: Sender<Command>,
    cancel: CancelFlag,
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
        let worker_cancel = cancel.clone();
        thread::Builder::new()
            .name(WORKER_NAME.to_owned())
            .spawn(move || {
                unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0) };
                Worker {
                    backend,
                    cancel: worker_cancel,
                    idle_unload,
                    refusing_loads: false,
                }
                .run(&receiver);
            })
            .expect("the generation worker starts");
        Self { commands, cancel }
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
        if level != PressureLevel::Normal {
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
    idle_unload: Option<Duration>,
    refusing_loads: bool,
}

impl Worker {
    fn run(mut self, receiver: &Receiver<Command>) {
        loop {
            let command = match self.idle_unload.filter(|_| self.backend.is_loaded()) {
                Some(idle) => match receiver.recv_timeout(idle) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => {
                        self.backend.unload();
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

    fn execute(&mut self, command: Command) {
        match command {
            Command::Load {
                weights,
                class,
                context_length,
                reply,
            } => {
                if self.refusing_loads {
                    reply
                        .send(&BackendError::new(ErrorCode::OutOfMemory, "memory pressure is critical").into_message());
                    return;
                }
                self.cancel.lower();
                match self.backend.load(weights, class, context_length, &self.cancel) {
                    Ok(()) => reply.send(&Message::Ready),
                    Err(error) => reply.send(&error.into_message()),
                }
            }
            Command::Unload { reply } => {
                self.backend.unload();
                reply.send(&Message::Ready);
            }
            Command::Generate { request, reply } => {
                self.cancel.lower();
                let outcome = self.backend.generate(&request, &self.cancel, &mut |text| {
                    reply.stream(&Message::Token { text: text.to_owned() });
                });
                match outcome {
                    Ok(stats) => reply.send(&Message::Done(stats)),
                    Err(error) => reply.send(&error.into_message()),
                }
            }
            Command::CountTokens { text, reply } => match self.backend.count_tokens(&text) {
                Ok(count) => reply.send(&Message::TokenCount { count }),
                Err(error) => reply.send(&error.into_message()),
            },
            Command::Pressure(PressureLevel::Warning) => self.backend.unload(),
            Command::Pressure(PressureLevel::Critical) => {
                self.backend.unload();
                self.refusing_loads = true;
            }
            Command::Pressure(PressureLevel::Normal) => self.refusing_loads = false,
        }
    }
}
