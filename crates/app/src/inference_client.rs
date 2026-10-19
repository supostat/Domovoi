use std::ffi::CStr;
use std::os::fd::OwnedFd;
use std::process;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_inference::{BackendError, CancelFlag, InferenceBackend};
use domovoi_inference_proto::{ErrorCode, GenerateRequest, GenerationStats, Message, ModelClass, Timeouts};
use domovoi_log_sys::SystemLog;

use crate::loaded_weights::LoadedWeights;
use crate::service_address::ServiceAddress;
use crate::service_link::ServiceLink;

const POLL: Duration = Duration::from_millis(20);
const TOKEN_SETTLE: Duration = Duration::from_millis(100);
const STALLED: &str = "the inference service stopped answering and was stopped";
const LOG_SUBSYSTEM: &CStr = c"com.domovoi.app";
const LOG_CATEGORY: &CStr = c"inference";

#[derive(Debug)]
pub struct InferenceClient {
    address: ServiceAddress,
    link: ServiceLink,
    tokens: Receiver<String>,
    token_sender: Sender<String>,
    timeouts: Timeouts,
    service_pid: i32,
    loaded: Option<LoadedWeights>,
    log: SystemLog,
}

impl InferenceClient {
    pub fn connect(address: ServiceAddress, timeouts: Timeouts) -> Result<Self, BackendError> {
        let (token_sender, tokens) = mpsc::channel();
        let link = ServiceLink::open(&address, token_sender.clone()).map_err(unavailable)?;
        let mut client = Self {
            address,
            link,
            tokens,
            token_sender,
            timeouts,
            service_pid: 0,
            loaded: None,
            log: SystemLog::new(LOG_SUBSYSTEM, LOG_CATEGORY),
        };
        client.service_pid = client.handshake()?;
        Ok(client)
    }

    pub fn service_pid(&self) -> i32 {
        self.service_pid
    }

    pub fn reconnect(&mut self) -> Result<(), BackendError> {
        self.link.close();
        self.link = ServiceLink::open(&self.address, self.token_sender.clone()).map_err(unavailable)?;
        self.service_pid = self.handshake()?;
        self.reload(&CancelFlag::default())
    }

    fn reload(&mut self, cancel: &CancelFlag) -> Result<(), BackendError> {
        let Some(loaded) = self.loaded.take() else {
            return Ok(());
        };
        let weights = loaded
            .weights
            .try_clone()
            .map_err(|error| internal(error.to_string()))?;
        self.load_weights(weights, loaded.class, loaded.context_length, cancel)
    }

    fn unloaded_by_service(&self, error: &BackendError) -> bool {
        error.code == ErrorCode::ModelNotLoaded && self.loaded.is_some()
    }

    fn restore_unloaded(&mut self, cancel: &CancelFlag) -> Result<(), BackendError> {
        self.log
            .notice("the inference service unloaded the model; the weights are loaded again");
        self.reload(cancel)
    }

    fn handshake(&mut self) -> Result<i32, BackendError> {
        let own = i32::try_from(process::id()).expect("a process id fits in pid_t");
        match self.exchange(
            Message::Handshake { pid: own },
            self.timeouts.handshake,
            None,
            &mut |_| {},
        )? {
            Message::Handshake { pid } => Ok(pid),
            other => Err(unexpected(&other)),
        }
    }

    fn load_weights(
        &mut self,
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
        cancel: &CancelFlag,
    ) -> Result<(), BackendError> {
        let kept = weights.try_clone().map_err(|error| internal(error.to_string()))?;
        let message = Message::Load {
            weights,
            class,
            context_length,
        };
        match self.exchange(message, self.timeouts.load, Some(cancel), &mut |_| {})? {
            Message::Ready => {
                self.loaded = Some(LoadedWeights {
                    weights: kept,
                    class,
                    context_length,
                });
                Ok(())
            }
            Message::Error { code, description } => Err(self.service_error(code, description)),
            other => Err(unexpected(&other)),
        }
    }

    fn generate_once(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        match self.exchange(
            Message::Generate(request.clone()),
            self.timeouts.stall,
            Some(cancel),
            on_token,
        )? {
            Message::Done(stats) => Ok(stats),
            Message::Error { code, description } => Err(self.service_error(code, description)),
            other => Err(unexpected(&other)),
        }
    }

    fn count_tokens_once(&self, text: &str) -> Result<u32, BackendError> {
        let (reply_sender, replies) = mpsc::channel();
        let link = self.link.clone();
        let message = Message::CountTokens { text: text.to_owned() };
        thread::spawn(move || {
            let _ = reply_sender.send(link.request(&message));
        });
        match replies.recv_timeout(self.timeouts.stall) {
            Ok(Ok(Message::TokenCount { count })) => Ok(count),
            Ok(Ok(Message::Error { code, description })) => Err(self.service_error(code, description)),
            Ok(Ok(other)) => Err(unexpected(&other)),
            Ok(Err(description)) => Err(self.link_failure(description)),
            Err(_) => {
                self.terminate();
                Err(stalled())
            }
        }
    }

    fn exchange(
        &mut self,
        message: Message,
        limit: Duration,
        cancel: Option<&CancelFlag>,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<Message, BackendError> {
        while self.tokens.try_recv().is_ok() {}
        let (reply_sender, replies) = mpsc::channel();
        let link = self.link.clone();
        thread::spawn(move || {
            let _ = reply_sender.send(link.request(&message));
        });
        let mut last_activity = Instant::now();
        let mut cancel_sent = false;
        loop {
            for text in self.tokens.try_iter() {
                on_token(&text);
                last_activity = Instant::now();
            }
            match replies.recv_timeout(POLL) {
                Ok(Ok(reply)) => {
                    self.settle_tokens(on_token);
                    return Ok(reply);
                }
                Ok(Err(description)) => {
                    let failure = self.link_failure(description);
                    let recovered = self.reconnect();
                    return Err(recovered.err().unwrap_or(failure));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(internal("the request thread stopped")),
            }
            if !cancel_sent && cancel.is_some_and(CancelFlag::is_raised) {
                self.send_cancel();
                cancel_sent = true;
                last_activity = Instant::now();
            }
            if last_activity.elapsed() > limit {
                self.terminate();
                let _ = replies.recv_timeout(self.timeouts.handshake);
                let recovered = self.reconnect();
                return Err(recovered.err().unwrap_or_else(stalled));
            }
        }
    }

    fn settle_tokens(&self, on_token: &mut dyn FnMut(&str)) {
        while let Ok(text) = self.tokens.recv_timeout(TOKEN_SETTLE) {
            on_token(&text);
        }
    }

    fn send_cancel(&self) {
        let link = self.link.clone();
        thread::spawn(move || {
            let _ = link.request(&Message::Cancel);
        });
    }

    fn terminate(&self) {
        self.log.error(&format!(
            "the inference service (pid {}) stopped answering and was stopped",
            self.service_pid
        ));
        if self.service_pid > 0 {
            unsafe { libc::kill(self.service_pid, libc::SIGKILL) };
        }
    }

    fn service_error(&self, code: ErrorCode, description: String) -> BackendError {
        self.log
            .error(&format!("the inference service answered {code:?}: {description}"));
        BackendError::new(code, description)
    }

    fn link_failure(&self, description: String) -> BackendError {
        self.log
            .error(&format!("the link to the inference service failed: {description}"));
        internal(description)
    }
}

impl InferenceBackend for InferenceClient {
    fn load(
        &mut self,
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
        cancel: &CancelFlag,
    ) -> Result<(), BackendError> {
        self.load_weights(weights, class, context_length, cancel)
    }

    fn unload(&mut self) {
        self.loaded = None;
        let _ = self.exchange(Message::Unload, self.timeouts.handshake, None, &mut |_| {});
    }

    fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }

    fn generate(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        match self.generate_once(request, cancel, on_token) {
            Err(error) if self.unloaded_by_service(&error) => {
                self.restore_unloaded(cancel)?;
                self.generate_once(request, cancel, on_token)
            }
            outcome => outcome,
        }
    }

    fn count_tokens(&mut self, text: &str) -> Result<u32, BackendError> {
        match self.count_tokens_once(text) {
            Err(error) if self.unloaded_by_service(&error) => {
                self.restore_unloaded(&CancelFlag::default())?;
                self.count_tokens_once(text)
            }
            outcome => outcome,
        }
    }
}

fn stalled() -> BackendError {
    BackendError::new(ErrorCode::Stalled, STALLED)
}

fn unavailable(description: String) -> BackendError {
    BackendError::new(ErrorCode::Unavailable, description)
}

fn internal(description: impl Into<String>) -> BackendError {
    BackendError::new(ErrorCode::Internal, description)
}

fn unexpected(message: &Message) -> BackendError {
    internal(format!("the service answered {}", message.kind().to_string_lossy()))
}
