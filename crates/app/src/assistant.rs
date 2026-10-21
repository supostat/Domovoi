use std::path::Path;
use std::time::Duration;

use domovoi_agent::FocusContext;
use domovoi_downloader::{Catalog, CatalogEntry, ModelStore};
use domovoi_inference::InferenceBackend;
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::ai_models::AiModels;
use crate::ask_job::AskJob;
use crate::ask_outcome::AskOutcome;
use crate::assistant_settings::AssistantSettings;
use crate::availability::Availability;
use crate::chat_model::ChatModel;
use crate::inference_client::InferenceClient;
use crate::model_library::ModelLibrary;
use crate::question::Question;
use crate::session::Session;

#[derive(Debug)]
pub(crate) struct Assistant {
    pub settings: AssistantSettings,
    pub library: ModelLibrary,
    pub chat: ChatModel,
    pub job: Option<AskJob>,
    pub library_timer: Option<Retained<NSTimer>>,
    unload_timer: Option<Retained<NSTimer>>,
    client: Option<InferenceClient>,
    loaded_entry: Option<String>,
}

impl Assistant {
    pub(crate) fn new(store: ModelStore, catalog: Catalog, settings: AssistantSettings) -> Self {
        Self {
            library: ModelLibrary::new(store, catalog, settings.source()),
            settings,
            chat: ChatModel::default(),
            job: None,
            library_timer: None,
            unload_timer: None,
            client: None,
            loaded_entry: None,
        }
    }

    pub(crate) fn active_model(&self) -> Option<(&CatalogEntry, &Path)> {
        self.library.installed(self.settings.active_entry.as_deref()?)
    }

    pub(crate) fn availability(&self) -> Availability {
        if !self.settings.enabled {
            Availability::Off
        } else if self.active_model().is_none() {
            Availability::NoModel
        } else if let Some(job) = &self.job {
            if job.loaded {
                Availability::Answering
            } else {
                Availability::Loading
            }
        } else if self.loaded().is_some() {
            Availability::Ready
        } else {
            Availability::Unloaded
        }
    }

    pub(crate) fn models(&self) -> AiModels {
        let mut chat = self.chat.clone();
        chat.availability = self.availability();
        AiModels {
            chat,
            model: self.library.screen_model(&self.settings),
        }
    }

    pub(crate) fn adopt_first_installed(&mut self) -> bool {
        if self.active_model().is_some() {
            return false;
        }
        let Some(first) = self.library.first_installed() else {
            return false;
        };
        self.settings.active_entry = Some(first.to_owned());
        true
    }

    pub(crate) fn begin(&mut self, session: Session, question: String, focus: FocusContext, timer: Retained<NSTimer>) {
        let (model, weights) = self
            .active_model()
            .map(|(entry, path)| (entry.clone(), path.to_path_buf()))
            .expect("questions are asked with an active model");
        let reload = self.loaded() != Some(model.id.as_str());
        self.cancel_unload();
        let history = self.chat.past_turns();
        self.chat.ask(&question);
        self.chat.draft.clear();
        self.job = Some(AskJob::start(
            session,
            self.client.take(),
            model,
            reload.then_some(weights),
            Question {
                text: question,
                history,
                focus,
            },
            timer,
        ));
    }

    pub(crate) fn finish(&mut self, outcome: AskOutcome) -> Session {
        let active = self.settings.active_entry.clone();
        match &outcome.answer {
            Ok(answer) => self.chat.record_answer(answer),
            Err(error) => self.chat.record_failure(error),
        }
        self.loaded_entry = outcome.client.as_ref().filter(|client| client.is_loaded()).and(active);
        self.client = outcome.client;
        outcome.session
    }

    pub(crate) fn unload_after(&self) -> Option<Duration> {
        if self.job.is_some() || self.loaded().is_none() {
            return None;
        }
        self.settings.unload.after()
    }

    pub(crate) fn schedule_unload(&mut self, timer: Option<Retained<NSTimer>>) {
        self.cancel_unload();
        self.unload_timer = timer;
    }

    pub(crate) fn unload(&mut self) {
        self.cancel_unload();
        if self.job.is_some() {
            return;
        }
        if let Some(client) = self.client.as_mut().filter(|client| client.is_loaded()) {
            client.unload();
        }
        self.loaded_entry = None;
    }

    pub(crate) fn shutdown(&mut self) {
        if let Some(job) = &self.job {
            job.stop();
        }
        self.library.pause_all();
        self.cancel_unload();
    }

    fn loaded(&self) -> Option<&str> {
        self.client.as_ref().filter(|client| client.is_loaded())?;
        self.loaded_entry.as_deref()
    }

    fn cancel_unload(&mut self) {
        if let Some(timer) = self.unload_timer.take() {
            timer.invalidate();
        }
    }
}
