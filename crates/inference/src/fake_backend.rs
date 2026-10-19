use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::os::fd::OwnedFd;
use std::process;
use std::thread;
use std::time::{Duration, Instant};

use domovoi_inference_proto::{ErrorCode, GenerateRequest, GenerationStats, ModelClass, StopReason};

use crate::backend_error::BackendError;
use crate::cancel_flag::CancelFlag;
use crate::fake_command::FakeCommand;
use crate::inference_backend::InferenceBackend;

const WEIGHTS_PREFIX: usize = 4;
const CANCEL_POLL: Duration = Duration::from_millis(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub struct FakeBackend {
    weights_prefix: Option<[u8; WEIGHTS_PREFIX]>,
    replies: VecDeque<String>,
    requests: Vec<GenerateRequest>,
}

impl FakeBackend {
    pub fn replying(replies: impl IntoIterator<Item = String>) -> Self {
        Self {
            replies: replies.into_iter().collect(),
            ..Self::default()
        }
    }

    pub fn requests(&self) -> &[GenerateRequest] {
        &self.requests
    }
}

impl InferenceBackend for FakeBackend {
    fn load(
        &mut self,
        weights: OwnedFd,
        _class: ModelClass,
        _context_length: u32,
        _cancel: &CancelFlag,
    ) -> Result<(), BackendError> {
        let mut prefix = [0_u8; WEIGHTS_PREFIX];
        File::from(weights)
            .read_exact(&mut prefix)
            .map_err(|error| BackendError::new(ErrorCode::LoadFailed, error.to_string()))?;
        self.weights_prefix = Some(prefix);
        Ok(())
    }

    fn unload(&mut self) {
        self.weights_prefix = None;
    }

    fn is_loaded(&self) -> bool {
        self.weights_prefix.is_some()
    }

    fn generate(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        let prefix = self
            .weights_prefix
            .ok_or_else(|| BackendError::new(ErrorCode::ModelNotLoaded, "no model is loaded"))?;
        self.requests.push(request.clone());
        let script = self.replies.pop_front().unwrap_or_else(|| request.prompt.clone());
        let commands = script
            .lines()
            .map(|line| {
                FakeCommand::parse(line)
                    .ok_or_else(|| BackendError::new(ErrorCode::Internal, format!("unknown script line {line:?}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let started = Instant::now();
        let mut generated_tokens = 0;
        let mut stop = StopReason::EndOfGeneration;
        for command in commands {
            if cancel.is_raised() {
                stop = StopReason::Cancelled;
                break;
            }
            match command {
                FakeCommand::Token(text) => {
                    generated_tokens += 1;
                    on_token(&text);
                }
                FakeCommand::Delay(duration) => {
                    if !wait(duration, cancel) {
                        stop = StopReason::Cancelled;
                        break;
                    }
                }
                FakeCommand::Fail(code, description) => return Err(BackendError::new(code, description)),
                FakeCommand::Hang => {
                    while !cancel.is_raised() {
                        thread::sleep(CANCEL_POLL);
                    }
                    stop = StopReason::Cancelled;
                    break;
                }
                FakeCommand::Crash => process::abort(),
                FakeCommand::ProbeNetwork(address) => on_token(&probe(&address)),
                FakeCommand::Weights => on_token(&String::from_utf8_lossy(&prefix)),
            }
        }
        Ok(GenerationStats {
            prompt_tokens: u32::try_from(request.prompt.lines().count()).unwrap_or(u32::MAX),
            generated_tokens,
            prompt_duration: Duration::ZERO,
            generation_duration: started.elapsed(),
            stop,
        })
    }

    fn count_tokens(&mut self, text: &str) -> Result<u32, BackendError> {
        if self.weights_prefix.is_none() {
            return Err(BackendError::new(ErrorCode::ModelNotLoaded, "no model is loaded"));
        }
        Ok(u32::try_from(text.len()).unwrap_or(u32::MAX))
    }
}

fn wait(duration: Duration, cancel: &CancelFlag) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if cancel.is_raised() {
            return false;
        }
        thread::sleep(CANCEL_POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
    true
}

fn probe(address: &str) -> String {
    let target = match address.to_socket_addrs().map(|mut targets| targets.next()) {
        Ok(Some(target)) => target,
        Ok(None) => return "network: no address".to_owned(),
        Err(error) => return format!("network: {error}"),
    };
    match TcpStream::connect_timeout(&target, CONNECT_TIMEOUT) {
        Ok(_) => "network: connected".to_owned(),
        Err(error) => format!("network: {error}"),
    }
}
