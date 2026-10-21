use std::path::Path;
use std::time::Duration;

#[cfg(feature = "claude-code")]
use domovoi_agent::ClaudeCode;
use domovoi_agent::FocusContext;
use domovoi_downloader::{Catalog, CatalogEntry, ModelStore};
use domovoi_inference::InferenceBackend;
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::ai_models::AiModels;
use crate::ask_job::AskJob;
use crate::ask_outcome::AskOutcome;
#[cfg(feature = "claude-code")]
use crate::assistant_provider::AssistantProvider;
use crate::assistant_settings::AssistantSettings;
use crate::availability::Availability;
use crate::chat_model::ChatModel;
#[cfg(feature = "claude-code")]
use crate::claude_code_choice::ClaudeCodeChoice;
use crate::inference_client::InferenceClient;
use crate::model_library::ModelLibrary;
#[cfg(feature = "claude-code")]
use crate::model_screen_model::ModelScreenModel;
use crate::question::Question;
use crate::session::Session;

#[derive(Debug)]
pub struct Assistant {
    pub(crate) settings: AssistantSettings,
    pub(crate) library: ModelLibrary,
    pub(crate) chat: ChatModel,
    pub(crate) job: Option<AskJob>,
    pub(crate) library_timer: Option<Retained<NSTimer>>,
    ask_timer: Option<Retained<NSTimer>>,
    unload_timer: Option<Retained<NSTimer>>,
    client: Option<InferenceClient>,
    loaded_entry: Option<String>,
    #[cfg(feature = "claude-code")]
    claude: Option<ClaudeCode>,
}

impl Assistant {
    pub fn new(store: ModelStore, catalog: Catalog, settings: AssistantSettings) -> Self {
        Self {
            library: ModelLibrary::new(store, catalog, settings.source()),
            settings,
            chat: ChatModel::default(),
            job: None,
            library_timer: None,
            ask_timer: None,
            unload_timer: None,
            client: None,
            loaded_entry: None,
            #[cfg(feature = "claude-code")]
            claude: None,
        }
    }

    #[cfg(feature = "claude-code")]
    #[must_use]
    pub fn with_claude_code(self, claude: Option<ClaudeCode>) -> Self {
        Self { claude, ..self }
    }

    pub(crate) fn active_model(&self) -> Option<(&CatalogEntry, &Path)> {
        self.library.installed(self.settings.active_entry.as_deref()?)
    }

    pub fn availability(&self) -> Availability {
        if !self.settings.enabled {
            return Availability::Off;
        }
        #[cfg(feature = "claude-code")]
        if self.chosen_claude_code().is_some() {
            return if self.job.is_some() {
                Availability::Answering
            } else {
                Availability::Ready
            };
        }
        if self.active_model().is_none() {
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

    pub fn models(&self) -> AiModels {
        let mut chat = self.chat.clone();
        chat.availability = self.availability();
        let model = self.library.screen_model(&self.settings);
        #[cfg(feature = "claude-code")]
        let model = ModelScreenModel {
            claude_code: Some(ClaudeCodeChoice {
                found: self.claude.is_some(),
                chosen: self.settings.provider == AssistantProvider::ClaudeCode,
            }),
            ..model
        };
        AiModels { chat, model }
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
        self.ask_timer = Some(timer);
        let question = Question {
            history: self.chat.past_turns(),
            text: question,
            focus,
        };
        self.chat.ask(&question.text);
        self.chat.draft.clear();
        #[cfg(feature = "claude-code")]
        if let Some(claude) = self.chosen_claude_code().cloned() {
            self.job = Some(AskJob::start_with_claude(
                session,
                self.client.take(),
                claude,
                question,
                self.chat.claude_session.clone(),
            ));
            return;
        }
        let (model, weights) = self
            .active_model()
            .map(|(entry, path)| (entry.clone(), path.to_path_buf()))
            .expect("questions are asked with an active model");
        let reload = self.loaded() != Some(model.id.as_str());
        self.cancel_unload();
        self.job = Some(AskJob::start(
            session,
            self.client.take(),
            model,
            reload.then_some(weights),
            question,
        ));
    }

    pub(crate) fn finish(&mut self, outcome: AskOutcome) -> Session {
        if let Some(timer) = self.ask_timer.take() {
            timer.invalidate();
        }
        let active = self.settings.active_entry.clone();
        outcome.reply.record(&mut self.chat);
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

    #[cfg(feature = "claude-code")]
    fn chosen_claude_code(&self) -> Option<&ClaudeCode> {
        self.claude
            .as_ref()
            .filter(|_| self.settings.provider == AssistantProvider::ClaudeCode)
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
