#[cfg(feature = "claude-code")]
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use domovoi_agent::{AgentError, AgentLimits};
#[cfg(feature = "claude-code")]
use domovoi_agent::{ClaudeCode, ClaudeCodeError};
use domovoi_downloader::CatalogEntry;
use domovoi_inference::{BackendError, CancelFlag, InferenceBackend};
use domovoi_inference_proto::{ErrorCode, Timeouts};

use crate::ask_event::AskEvent;
use crate::ask_outcome::AskOutcome;
use crate::ask_reply::AskReply;
use crate::chat_model::ChatModel;
use crate::inference_client::InferenceClient;
use crate::question::Question;
use crate::service_address::ServiceAddress;
use crate::session::Session;

#[derive(Debug)]
pub struct AskJob {
    thread: Option<JoinHandle<AskOutcome>>,
    events: Receiver<AskEvent>,
    cancel: CancelFlag,
    pub loaded: bool,
}

impl AskJob {
    pub fn start(
        session: Session,
        client: Option<InferenceClient>,
        model: CatalogEntry,
        weights: Option<PathBuf>,
        question: Question,
    ) -> Self {
        Self::spawned(move |cancel, events| answer(session, client, &model, weights, &question, cancel, events))
    }

    #[cfg(feature = "claude-code")]
    pub fn start_with_claude(
        session: Session,
        client: Option<InferenceClient>,
        claude: ClaudeCode,
        question: Question,
        claude_session: Option<String>,
    ) -> Self {
        Self::spawned(move |cancel, events| {
            answer_with_claude(
                session,
                client,
                &claude,
                &question,
                claude_session.as_deref(),
                cancel,
                events,
            )
        })
    }

    pub fn absorb(&mut self, chat: &mut ChatModel) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            match event {
                AskEvent::Loaded => self.loaded = true,
                AskEvent::Step(step) => chat.record_step(&step),
            }
            changed = true;
        }
        changed
    }

    pub fn stop(&self) {
        self.cancel.raise();
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub fn finish(mut self) -> AskOutcome {
        self.thread
            .take()
            .expect("a question is answered once")
            .join()
            .expect("the answer thread does not panic")
    }

    fn spawned(work: impl FnOnce(&CancelFlag, &Sender<AskEvent>) -> AskOutcome + Send + 'static) -> Self {
        let (sender, events) = mpsc::channel();
        let cancel = CancelFlag::default();
        let thread = {
            let cancel = cancel.clone();
            thread::spawn(move || work(&cancel, &sender))
        };
        Self {
            thread: Some(thread),
            events,
            cancel,
            loaded: false,
        }
    }
}

fn answer(
    session: Session,
    client: Option<InferenceClient>,
    model: &CatalogEntry,
    weights: Option<PathBuf>,
    question: &Question,
    cancel: &CancelFlag,
    events: &Sender<AskEvent>,
) -> AskOutcome {
    let connected = match client {
        Some(client) => Ok(client),
        None => Timeouts::from_environment()
            .map_err(|error| BackendError::new(ErrorCode::Unavailable, error.to_string()))
            .and_then(|timeouts| InferenceClient::connect(ServiceAddress::bundled(), timeouts)),
    };
    let mut client = match connected {
        Ok(client) => client,
        Err(error) => {
            return AskOutcome {
                session,
                client: None,
                reply: AskReply::Local(Err(AgentError::Backend(error))),
            };
        }
    };
    if let Some(weights) = weights {
        let loaded = File::open(&weights)
            .map_err(|error| BackendError::new(ErrorCode::LoadFailed, error.to_string()))
            .and_then(|file| client.load(file.into(), model.class, model.context_length, cancel));
        if let Err(error) = loaded {
            return AskOutcome {
                session,
                client: Some(client),
                reply: AskReply::Local(Err(AgentError::Backend(error))),
            };
        }
    }
    let _ = events.send(AskEvent::Loaded);
    let limits = AgentLimits::for_context_length(model.context_length);
    let answer = session.ask(&mut client, model, limits, question, cancel, &mut |step| {
        let _ = events.send(AskEvent::Step(step.clone()));
    });
    AskOutcome {
        session,
        client: Some(client),
        reply: AskReply::Local(answer),
    }
}

#[cfg(feature = "claude-code")]
fn answer_with_claude(
    session: Session,
    client: Option<InferenceClient>,
    claude: &ClaudeCode,
    question: &Question,
    claude_session: Option<&str>,
    cancel: &CancelFlag,
    events: &Sender<AskEvent>,
) -> AskOutcome {
    let _ = events.send(AskEvent::Loaded);
    let answer = fs::create_dir_all(&claude.workspace)
        .map_err(ClaudeCodeError::Process)
        .and_then(|()| {
            session.ask_claude(claude, question, claude_session, cancel, &mut |step| {
                let _ = events.send(AskEvent::Step(step.clone()));
            })
        });
    AskOutcome {
        session,
        client,
        reply: AskReply::Claude(answer),
    }
}
