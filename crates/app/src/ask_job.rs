use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use domovoi_agent::{AgentError, AgentLimits};
use domovoi_downloader::CatalogEntry;
use domovoi_inference::{BackendError, CancelFlag, InferenceBackend};
use domovoi_inference_proto::{ErrorCode, Timeouts};
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::ask_event::AskEvent;
use crate::ask_outcome::AskOutcome;
use crate::chat_model::ChatModel;
use crate::inference_client::InferenceClient;
use crate::question::Question;
use crate::service_address::ServiceAddress;
use crate::session::Session;

#[derive(Debug)]
pub(crate) struct AskJob {
    thread: Option<JoinHandle<AskOutcome>>,
    events: Receiver<AskEvent>,
    cancel: CancelFlag,
    timer: Retained<NSTimer>,
    pub loaded: bool,
}

impl AskJob {
    pub(crate) fn start(
        session: Session,
        client: Option<InferenceClient>,
        model: CatalogEntry,
        weights: Option<PathBuf>,
        question: Question,
        timer: Retained<NSTimer>,
    ) -> Self {
        let (sender, events) = mpsc::channel();
        let cancel = CancelFlag::default();
        let thread = {
            let cancel = cancel.clone();
            thread::spawn(move || answer(session, client, &model, weights, &question, &cancel, &sender))
        };
        Self {
            thread: Some(thread),
            events,
            cancel,
            timer,
            loaded: false,
        }
    }

    pub(crate) fn absorb(&mut self, chat: &mut ChatModel) -> bool {
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

    pub(crate) fn stop(&self) {
        self.cancel.raise();
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub(crate) fn finish(mut self) -> AskOutcome {
        self.timer.invalidate();
        self.thread
            .take()
            .expect("a question is answered once")
            .join()
            .expect("the answer thread does not panic")
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
                answer: Err(AgentError::Backend(error)),
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
                answer: Err(AgentError::Backend(error)),
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
        answer,
    }
}
