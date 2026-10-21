use std::path::PathBuf;
use std::time::{Duration, Instant};

use domovoi_agent::FocusContext;
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem,
    NSModalResponseOK, NSOpenPanel, NSTextField,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use super::ShellController;
#[cfg(feature = "claude-code")]
use crate::assistant_provider::AssistantProvider;
use crate::assistant_settings::AssistantSettings;
use crate::chat_entry::ChatEntry;
use crate::install_error_text::install_error_text;
use crate::screen_views::ScreenViews;
use crate::section::Section;
use crate::stage::Stage;
use crate::strings::{CANCEL, SOURCE_HINT, SOURCE_PROMPT, SOURCE_SAVE};
use crate::unload_delay::UnloadDelay;

const LIBRARY_TICK: f64 = 1.0;
const ASK_TICK: f64 = 1.0 / 10.0;
const MENU_OFFSET: f64 = 4.0;
const SOURCE_FIELD: NSSize = NSSize::new(320.0, 24.0);

impl ShellController {
    pub(super) fn watch_library(&self) {
        let needs_timer = {
            let state = self.state.borrow();
            state.assistant.library.is_busy() && state.assistant.library_timer.is_none()
        };
        if needs_timer {
            let timer = self.repeating(LIBRARY_TICK, ShellController::library_tick);
            self.state.borrow_mut().assistant.library_timer = Some(timer);
        }
    }

    fn library_tick(&self) {
        let (changed_index, problems) = {
            let mut state = self.state.borrow_mut();
            let (events, problems) = state.assistant.library.poll(Instant::now());
            if !state.assistant.library.is_busy()
                && let Some(timer) = state.assistant.library_timer.take()
            {
                timer.invalidate();
            }
            if state.assistant.adopt_first_installed() {
                state.assistant.settings.save();
            }
            let changed_index = !events.is_empty();
            state.pending_model_events.extend(events);
            (changed_index, problems)
        };
        self.show_assistant_change(changed_index);
        for problem in problems {
            self.inform(&problem);
        }
    }

    fn show_assistant_change(&self, changed_index: bool) {
        let (main, section, session_open) = {
            let state = self.state.borrow();
            (state.stage == Stage::Main, state.section, state.session.is_some())
        };
        if changed_index && main && session_open {
            self.refresh();
        } else if main && matches!(section, Section::Assistant | Section::Model) {
            self.show_current();
        }
    }

    pub(super) fn ask_assistant(&self) {
        let question = {
            let state = self.state.borrow();
            match &state.views {
                Some(ScreenViews::Assistant(screen)) => screen.draft().trim().to_owned(),
                _ => return,
            }
        };
        let ready = {
            let state = self.state.borrow();
            state.stage == Stage::Main && state.session.is_some() && state.assistant.availability().accepts_questions()
        };
        if question.is_empty() || !ready {
            return;
        }
        let timer = self.repeating(ASK_TICK, ShellController::ask_tick);
        {
            let mut state = self.state.borrow_mut();
            if let Some(ScreenViews::Assistant(screen)) = &state.views {
                screen.clear_draft();
            }
            let focus = FocusContext {
                folder: state.focus_path.clone(),
                selected: state
                    .care()
                    .recommendations
                    .cards
                    .iter()
                    .filter(|card| card.selected && card.selectable())
                    .flat_map(|card| card.paths.iter().cloned())
                    .collect(),
            };
            let session = state.session.take().expect("a question needs an open session");
            state.assistant.begin(session, question, focus, timer);
        }
        self.show_current();
    }

    fn ask_tick(&self) {
        let (finished, changed) = {
            let mut state = self.state.borrow_mut();
            let assistant = &mut state.assistant;
            let Some(job) = assistant.job.as_mut() else {
                return;
            };
            let changed = job.absorb(&mut assistant.chat);
            if job.is_finished() {
                (assistant.job.take(), changed)
            } else {
                (None, changed)
            }
        };
        match finished {
            Some(job) => {
                let outcome = job.finish();
                {
                    let mut state = self.state.borrow_mut();
                    let session = state.assistant.finish(outcome);
                    state.session = Some(session);
                }
                self.schedule_unload();
                self.refresh();
            }
            None if changed => self.show_current(),
            None => {}
        }
    }

    fn schedule_unload(&self) {
        let after = self.state.borrow().assistant.unload_after();
        match after {
            None => self.state.borrow_mut().assistant.schedule_unload(None),
            Some(Duration::ZERO) => self.state.borrow_mut().assistant.unload(),
            Some(delay) => {
                let timer = self.once(delay, ShellController::unload_tick);
                self.state.borrow_mut().assistant.schedule_unload(Some(timer));
            }
        }
    }

    fn unload_tick(&self) {
        self.state.borrow_mut().assistant.unload();
        self.show_assistant_change(false);
    }

    pub(super) fn stop_answer(&self) {
        if let Some(job) = &self.state.borrow().assistant.job {
            job.stop();
        }
    }

    pub(super) fn add_answer_to_tray(&self, position: usize) {
        let added = {
            let mut state = self.state.borrow_mut();
            let targets = match state.assistant.chat.entries.get(position) {
                Some(ChatEntry::Answer { targets, .. }) => targets.clone(),
                _ => return,
            };
            let Some(session) = state.session.as_mut() else {
                return;
            };
            let Some(index) = session.index() else {
                return;
            };
            let nodes: Vec<_> = targets
                .iter()
                .filter_map(|target| Some((index.find(&target.path)?, target.rule_id.clone())))
                .collect();
            for (node, rule_id) in &nodes {
                session.add_to_tray(*node, Some(rule_id));
            }
            !nodes.is_empty()
        };
        if added {
            self.open_tray();
        }
    }

    pub(super) fn toggle_assistant(&self) {
        {
            let mut state = self.state.borrow_mut();
            let assistant = &mut state.assistant;
            assistant.settings.enabled = !assistant.settings.enabled;
            assistant.settings.save();
            if !assistant.settings.enabled {
                if let Some(job) = &assistant.job {
                    job.stop();
                }
                assistant.unload();
            }
        }
        self.show_assistant_change(false);
    }

    #[cfg(feature = "claude-code")]
    pub(super) fn toggle_claude_code(&self) {
        {
            let mut state = self.state.borrow_mut();
            let settings = &mut state.assistant.settings;
            settings.provider = match settings.provider {
                AssistantProvider::LocalModel => AssistantProvider::ClaudeCode,
                AssistantProvider::ClaudeCode => AssistantProvider::LocalModel,
            };
            settings.save();
        }
        self.show_assistant_change(false);
    }

    pub(super) fn download_model(&self, position: usize) {
        self.state.borrow_mut().assistant.library.start(position);
        self.watch_library();
        self.show_assistant_change(false);
    }

    pub(super) fn pause_download(&self, position: usize) {
        self.state.borrow_mut().assistant.library.pause(position);
        self.show_assistant_change(false);
    }

    pub(super) fn cancel_download(&self, position: usize) {
        let cancelled = self.state.borrow_mut().assistant.library.cancel(position);
        if let Err(error) = cancelled {
            self.inform(install_error_text(&error));
        }
        self.show_assistant_change(false);
    }

    pub(super) fn remove_model(&self, position: usize) {
        let removed = {
            let mut state = self.state.borrow_mut();
            let entry_id = state
                .assistant
                .library
                .shown_entry(position)
                .map(|entry| entry.id.clone());
            let removed = state.assistant.library.remove(position);
            if let Ok(Some(event)) = &removed {
                state.pending_model_events.push(event.clone());
                let assistant = &mut state.assistant;
                if assistant.settings.active_entry.is_some() && assistant.settings.active_entry == entry_id {
                    assistant.unload();
                    assistant.settings.active_entry = None;
                    assistant.adopt_first_installed();
                    assistant.settings.save();
                }
            }
            removed
        };
        match removed {
            Ok(event) => self.show_assistant_change(event.is_some()),
            Err(error) => self.inform(install_error_text(&error)),
        }
    }

    pub(super) fn activate_model(&self, position: usize) {
        {
            let mut state = self.state.borrow_mut();
            let Some(entry_id) = state
                .assistant
                .library
                .shown_entry(position)
                .map(|entry| entry.id.clone())
            else {
                return;
            };
            let assistant = &mut state.assistant;
            assistant.unload();
            assistant.settings.active_entry = Some(entry_id);
            assistant.settings.save();
        }
        self.show_assistant_change(false);
    }

    pub(super) fn choose_unload_delay(&self) {
        let (button, current) = {
            let state = self.state.borrow();
            match &state.views {
                Some(ScreenViews::Model(screen)) => (screen.unload.clone(), state.assistant.settings.unload),
                _ => return,
            }
        };
        let menu = NSMenu::new(self.marker);
        for (position, delay) in UnloadDelay::ALL.into_iter().enumerate() {
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.marker),
                    &NSString::from_str(delay.title()),
                    Some(sel!(selectUnloadDelay:)),
                    &NSString::new(),
                )
            };
            unsafe { item.setTarget(Some(self.actions.target())) };
            item.setTag(isize::try_from(position).expect("five delays fit in isize"));
            item.setState(if delay == current {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            menu.addItem(&item);
        }
        let below = NSPoint::new(0.0, button.frame().size.height + MENU_OFFSET);
        menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(&button));
    }

    pub(super) fn select_unload_delay(&self, position: usize) {
        let Some(delay) = UnloadDelay::ALL.get(position).copied() else {
            return;
        };
        {
            let mut state = self.state.borrow_mut();
            state.assistant.settings.unload = delay;
            state.assistant.settings.save();
        }
        self.schedule_unload();
        self.show_assistant_change(false);
    }

    pub(super) fn change_source(&self) {
        let current = self
            .state
            .borrow()
            .assistant
            .settings
            .mirror
            .clone()
            .unwrap_or_default();
        let alert = NSAlert::new(self.marker);
        alert.setMessageText(&NSString::from_str(SOURCE_PROMPT));
        alert.setInformativeText(&NSString::from_str(SOURCE_HINT));
        let field = NSTextField::textFieldWithString(&NSString::from_str(&current), self.marker);
        field.setFrame(NSRect::new(NSPoint::ZERO, SOURCE_FIELD));
        alert.setAccessoryView(Some(&field));
        alert.addButtonWithTitle(&NSString::from_str(SOURCE_SAVE));
        alert.addButtonWithTitle(&NSString::from_str(CANCEL));
        alert.window().setInitialFirstResponder(Some(&field));
        if alert.runModal() != NSAlertFirstButtonReturn {
            return;
        }
        match AssistantSettings::parse_mirror(&field.stringValue().to_string()) {
            Ok(mirror) => {
                let mut state = self.state.borrow_mut();
                let assistant = &mut state.assistant;
                assistant.settings.mirror = mirror;
                assistant.settings.save();
                let source = assistant.settings.source();
                assistant.library.set_source(source);
            }
            Err(problem) => return self.inform(problem),
        }
        self.show_assistant_change(false);
    }

    pub(super) fn choose_model_file(&self) {
        let panel = NSOpenPanel::openPanel(self.marker);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(false);
        panel.setAllowsMultipleSelection(false);
        if panel.runModal() != NSModalResponseOK {
            return;
        }
        let Some(path) = panel
            .URL()
            .and_then(|url| url.path())
            .map(|path| PathBuf::from(path.to_string()))
        else {
            return;
        };
        let adopted = self.state.borrow_mut().assistant.library.adopt(path);
        match adopted {
            Ok(()) => {
                self.watch_library();
                self.show_assistant_change(false);
            }
            Err(problem) => self.inform(problem),
        }
    }

    pub(super) fn trash_orphan(&self, position: usize) {
        let trashed = self.state.borrow_mut().assistant.library.trash_orphan(position);
        if let Err(error) = trashed {
            self.inform(install_error_text(&error));
        }
        self.show_assistant_change(false);
    }
}
