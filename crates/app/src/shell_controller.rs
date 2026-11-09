use std::cell::RefCell;
use std::ffi::CStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant, SystemTime};

use block2::RcBlock;
use domovoi_downloader::{Catalog, ModelStore};
use domovoi_dups::SearchError;
use domovoi_index::Reclaimable;
use domovoi_log_sys::SystemLog;
use domovoi_render::{
    DiagramChange, DiagramGraphics, DiagramMode, DiagramObserver, DiagramRenderer, DiagramState, DiagramView,
    GlyphAtlas, NodeOrigin,
};
use domovoi_scanner::ScanError;
use domovoi_theme::{Palette, format_count, format_size, reduce_motion};
use objc2::rc::{Retained, autoreleasepool};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSApplicationTerminateReply, NSAutoresizingMaskOptions, NSBackingStoreType, NSModalResponseOK,
    NSPasteboard, NSPasteboardTypeString, NSSavePanel, NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification, NSWorkspaceOpenConfiguration,
};
use objc2_foundation::{
    NSArray, NSBundle, NSInteger, NSNotification, NSOperationQueue, NSPoint, NSRect, NSSize, NSString, NSTimer, NSURL,
};
use objc2_quick_look_ui::QLPreviewPanel;

use crate::access_screen::AccessScreen;
use crate::access_state::AccessState;
use crate::action_handler::ActionHandler;
use crate::apply_job::ApplyJob;
use crate::assistant::Assistant;
use crate::assistant_screen::AssistantScreen;
use crate::assistant_settings::AssistantSettings;
use crate::bundle_resources::diagram_metallib;
use crate::cache_opening::CacheOpening;
use crate::cached_tray_summary::CachedTraySummary;
use crate::card_model::CardModel;
use crate::care_models::CareModels;
#[cfg(feature = "claude-code")]
use crate::claude_search::installed_claude_code;
use crate::clone_refusal_text::clone_refusal_text;
use crate::csv_export::csv_lines;
use crate::delete_job::DeleteJob;
use crate::delete_outcome::DeleteOutcome;
use crate::delete_run::DeleteRun;
use crate::delete_status::DeleteStatus;
use crate::diagram_preference::{remember_mode, remember_small_groups, remembered_mode, remembered_small_groups};
use crate::disk_overview::{DiskOverview, SYSTEM_DATA_ROOT};
use crate::dock_progress::DockProgress;
use crate::drop_kind::DropKind;
use crate::duplicate_job::DuplicateJob;
use crate::duplicate_key::DuplicateKey;
use crate::duplicate_summary::DuplicateSummary;
use crate::duplicates_layout::DuplicatesLayout;
use crate::duplicates_model::DuplicatesModel;
use crate::duplicates_screen::DuplicatesScreen;
use crate::duplicates_state::DuplicatesState;
use crate::event_effect::EventEffect;
use crate::event_queue::{EventQueue, HEAVY_DELTA_NODES};
use crate::event_watch::EventWatch;
use crate::expansion::Expansion;
use crate::hover_panel::HoverPanel;
use crate::main_screen::MainScreen;
use crate::model_screen::ModelScreen;
use crate::navigation::Navigation;
use crate::node_facts::NodeFacts;
use crate::notification_center::ScanNotifier;
use crate::preview_source::PreviewSource;
use crate::preview_targets::PreviewTargets;
use crate::project_row::ProjectRow;
use crate::properties_panel::PropertiesPanel;
use crate::provenance::Provenance;
use crate::purge_job::{PURGE_TIMEOUT, PurgeJob, available_capacity};
use crate::purge_model::PurgeModel;
use crate::purge_progress::PurgeProgress;
use crate::purge_run::{PURGE_TICK, PurgeRun};
use crate::purge_screen::PurgeScreen;
use crate::purge_sheet::PurgeSheet;
use crate::quit_job::QuitJob;
use crate::recommendations_screen::RecommendationsScreen;
use crate::scan_job::ScanJob;
use crate::scan_notice::scan_finished_line;
use crate::scanning_screen::{ScanningScreen, center_lines};
use crate::screen_key::ScreenKey;
use crate::screen_views::ScreenViews;
use crate::section::Section;
use crate::selected_rows::selected_rows;
use crate::session::Session;
use crate::session_error::SessionError;
use crate::session_paths::SessionPaths;
use crate::shell_action::ShellAction;
use crate::shell_actions::ShellActions;
use crate::shell_data::ShellData;
use crate::shell_layout::WINDOW;
use crate::shell_palette::ShellPalette;
use crate::shell_root_view::ShellRootView;
use crate::shell_state::ShellState;
use crate::stage::Stage;
use crate::strings::{
    APP_NAME, CANCEL, CLONED, DELETE, DELETE_CONFIRM, DELETE_CONFIRM_HINT, DIAGRAM, FAILURE_CLOSE, FAILURE_TITLE,
    FDA_SETTINGS_URL, NOT_CLONED, OBJECTS, OPEN_DISK_UTILITY, PROTECTION_UNCHECKED, TERMINAL_MISSING, TMUTIL_COPIED,
    UNRESPONSIVE_TITLE, UPDATING_INDEX, csv_file_name, unresponsive_body,
};
use crate::tilde::tilde;
use crate::tmutil_commands::tmutil_commands;
use crate::tray_button::ESCAPE;
use crate::tray_measurement::TrayMeasurement;
use crate::tray_model::TrayModel;
use crate::tray_screen::TrayScreen;
use crate::tray_sheet::TraySheet;

const SCAN_TICK: f64 = 1.0 / 30.0;
const SEARCH_TICK: f64 = 1.0 / 10.0;
const EVENTS_TICK: f64 = 0.5;
const MEASURE_TICK: f64 = 0.25;
const TERMINAL_BUNDLE: &str = "com.apple.Terminal";
const DISK_UTILITY_BUNDLE: &str = "com.apple.DiskUtility";
const DELETE_TICK: f64 = 0.1;
const REFUSAL_SHOWN: Duration = Duration::from_secs(4);
const OPEN: &str = "/usr/bin/open";
const LOG_SUBSYSTEM: &CStr = c"com.domovoi.app";
const LOG_CATEGORY: &CStr = c"quit";

mod assistant_actions;

#[derive(Debug)]
pub(crate) struct ShellController {
    marker: MainThreadMarker,
    this: Weak<ShellController>,
    window: Retained<NSWindow>,
    root: Retained<ShellRootView>,
    actions: Retained<ShellActions>,
    hover: HoverPanel,
    properties: PropertiesPanel,
    preview: Retained<PreviewSource>,
    dock: RefCell<DockProgress>,
    notifier: Option<ScanNotifier>,
    paths: SessionPaths,
    state: RefCell<ShellState>,
}

impl ShellController {
    pub(crate) fn launch(marker: MainThreadMarker, paths: SessionPaths, models: ModelStore) -> Rc<Self> {
        let assistant = Assistant::new(models, Catalog::bundled(), AssistantSettings::load());
        #[cfg(feature = "claude-code")]
        let assistant = assistant.with_claude_code(installed_claude_code(&paths));
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(marker),
                NSRect::new(NSPoint::ZERO, WINDOW),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(APP_NAME));
        let root = ShellRootView::new(marker, NSRect::new(NSPoint::ZERO, WINDOW));
        window.setContentView(Some(&root));
        let controller = Rc::new_cyclic(|this| Self {
            marker,
            this: this.clone(),
            window,
            root,
            actions: ShellActions::new(marker),
            hover: HoverPanel::new(marker),
            properties: PropertiesPanel::new(marker),
            preview: PreviewSource::new(marker),
            dock: RefCell::new(DockProgress::new(marker)),
            notifier: ScanNotifier::in_bundle(),
            paths,
            state: RefCell::new(ShellState::new(remembered_mode(), remembered_small_groups(), assistant)),
        });
        let handler: Weak<dyn ActionHandler> = controller.this.clone();
        controller.actions.connect(handler);
        let appearance = controller.this.clone();
        controller.root.on_appearance_change(Rc::new(move || {
            if let Some(controller) = appearance.upgrade() {
                controller.restyle();
            }
        }));
        controller.root.set_preview_source(controller.preview.clone());
        controller.observe_contrast();
        controller.window.center();
        controller.window.makeKeyAndOrderFront(None);
        controller.watch_library();
        controller.start();
        controller
    }

    pub(crate) fn application_became_active(&self) {
        let returned_from_settings = {
            let state = self.state.borrow();
            state.settings_opened && matches!(state.stage, Stage::Access(_))
        };
        if returned_from_settings {
            self.state.borrow_mut().settings_opened = false;
            match AccessState::probe() {
                AccessState::Granted => self.open_session(),
                AccessState::Missing | AccessState::GrantedAfterLaunch => {
                    self.show_access(AccessState::GrantedAfterLaunch);
                }
            }
        }
    }

    pub(crate) fn dispatch(&self, action: ShellAction) {
        self.actions.dispatch(action);
    }

    pub(crate) fn small_groups_always(&self) -> bool {
        self.state.borrow().small_groups.always
    }

    fn toggle_small_groups(&self) {
        let mut state = self.state.borrow_mut();
        state.small_groups.always = !state.small_groups.always;
        let always = state.small_groups.always;
        remember_small_groups(always);
        if let Some(diagram) = state.views.as_ref().and_then(ScreenViews::diagram) {
            diagram.update_state(|diagram| diagram.set_small_groups_always(always));
        }
    }

    pub(crate) fn should_terminate(&self) -> NSApplicationTerminateReply {
        if self.state.borrow().quitting.is_some() {
            return NSApplicationTerminateReply::TerminateCancel;
        }
        let Some(session) = self.state.borrow_mut().session.take() else {
            return NSApplicationTerminateReply::TerminateNow;
        };
        self.hover.hide();
        self.properties.close();
        self.window.orderOut(None);
        self.stop_background_work();
        let marker = self.marker;
        let saving = QuitJob::start(session, move |saved| {
            if let Err(error) = saved {
                SystemLog::new(LOG_SUBSYSTEM, LOG_CATEGORY).error(&format!("The scan cache was not saved: {error}"));
            }
            NSApplication::sharedApplication(marker).replyToApplicationShouldTerminate(true);
        });
        self.state.borrow_mut().quitting = Some(saving);
        NSApplicationTerminateReply::TerminateLater
    }

    pub(crate) fn application_will_terminate(&self) {
        self.stop_background_work();
    }

    fn stop_background_work(&self) {
        self.dock.borrow_mut().clear();
        self.state.borrow_mut().assistant.shutdown();
        let mut state = self.state.borrow_mut();
        if let Some(job) = &state.scan {
            job.stop();
        }
        if let Some(job) = &state.search {
            job.stop();
        }
        if let Some(run) = state.delete.take() {
            run.stop();
            run.finish();
        }
        if let Some(run) = state.purge.as_mut().and_then(|sheet| sheet.run.take()) {
            run.stop();
            if let Err(error) = run.finish() {
                SystemLog::new(LOG_SUBSYSTEM, LOG_CATEGORY)
                    .error(&format!("The space release was not finished: {error}"));
            }
        }
    }

    fn start(&self) {
        match AccessState::probe() {
            AccessState::Granted => self.open_session(),
            access => self.show_access(access),
        }
    }

    fn open_session(&self) {
        let opened = DiskOverview::measure(Path::new(SYSTEM_DATA_ROOT)).and_then(|overview| {
            let session = Session::open(Path::new(SYSTEM_DATA_ROOT), &self.paths, env!("CARGO_PKG_VERSION"))?;
            Ok((overview, session))
        });
        let (overview, mut session) = match opened {
            Ok(opened) => opened,
            Err(error) => return self.fail(&error.to_string()),
        };
        self.state.borrow_mut().overview = Some(overview);
        match session.open_cached() {
            Ok(CacheOpening::Opened) => self.install(session),
            Ok(CacheOpening::Absent) => self.begin_scan(session, false),
            Ok(CacheOpening::Reset) => self.begin_scan(session, true),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn rescan(&self) {
        let overview = match DiskOverview::measure(Path::new(SYSTEM_DATA_ROOT)) {
            Ok(overview) => overview,
            Err(error) => return self.fail(&error.to_string()),
        };
        let session = {
            let mut state = self.state.borrow_mut();
            state.overview = Some(overview);
            state.session.take().expect("a rescan starts from an open session")
        };
        self.begin_scan(session, false);
    }

    fn begin_scan(&self, session: Session, full_first_scan: bool) {
        let access = AccessState::probe();
        let skip_access = self.state.borrow().scan_without_access;
        if access == AccessState::Missing && !skip_access {
            self.state.borrow_mut().session = Some(session);
            return self.show_access(access);
        }
        let timer = self.repeating(SCAN_TICK, ShellController::scan_tick);
        if let Some(notifier) = &self.notifier {
            notifier.request_authorization();
        }
        self.dock.borrow_mut().show(0.0);
        {
            let mut state = self.state.borrow_mut();
            state.watch = None;
            let expected = state.overview().volumes.data_used.value;
            state.scan = Some(ScanJob::start(session, full_first_scan, expected, timer));
            state.stage = Stage::Scanning;
            state.section = Section::Disk;
            state.access = access;
        }
        self.show_current();
    }

    fn scan_tick(&self) {
        let finished = {
            let mut state = self.state.borrow_mut();
            let ShellState { scan, views, .. } = &mut *state;
            let Some(job) = scan.as_mut() else {
                return;
            };
            if job.absorb_progress()
                && let Some(ScreenViews::Scanning { screen, diagram }) = views.as_ref()
            {
                let status = job.status.clone();
                screen.show_progress(&status);
                diagram.update_state(|diagram| {
                    diagram.set_sweep(status.sweep);
                    diagram.center = Some(center_lines(&status));
                });
                self.dock.borrow_mut().show(status.sweep);
            }
            if job.is_finished() { scan.take() } else { None }
        };
        if let Some(job) = finished {
            let status = job.status.clone();
            let elapsed = job.started.elapsed();
            let (session, outcome) = job.finish();
            self.dock.borrow_mut().clear();
            self.state.borrow_mut().last_scan = Some(status);
            match outcome {
                Ok(()) => {
                    if let Some(notifier) = &self.notifier {
                        notifier.scan_finished(self.marker, &scan_finished_line(elapsed));
                    }
                    self.install(session);
                }
                Err(SessionError::Scan(ScanError::Cancelled)) => self.install(session),
                Err(SessionError::Scan(ScanError::Unresponsive { path })) => {
                    self.fail_unresponsive(&path);
                    self.install(session);
                }
                Err(error) => {
                    self.state.borrow_mut().session = Some(session);
                    self.fail(&error.to_string());
                }
            }
        }
    }

    fn install(&self, session: Session) {
        let watch = match session.index() {
            Some(_) => match session.watch() {
                Ok((stream, events)) => Some(EventWatch {
                    _stream: stream,
                    events,
                    timer: self.repeating(EVENTS_TICK, ShellController::events_tick),
                    queue: EventQueue::new(HEAVY_DELTA_NODES),
                }),
                Err(error) => return self.fail(&error.to_string()),
            },
            None => None,
        };
        {
            let mut state = self.state.borrow_mut();
            state.session = Some(session);
            state.watch = watch;
            state.stage = Stage::Main;
            state.views = None;
        }
        self.refresh();
    }

    fn events_tick(&self) {
        let effect = if let Some(settled) = self.finish_apply() {
            settled
        } else {
            let mut state = self.state.borrow_mut();
            let ShellState { watch, session, .. } = &mut *state;
            let (Some(watch), Some(session)) = (watch.as_mut(), session.as_mut()) else {
                return;
            };
            watch.queue.advance(watch.events.try_iter(), session)
        };
        let lends_session = self.state.borrow().gate().lends_session();
        match effect {
            Ok(EventEffect::Applied) => self.refresh(),
            Ok(EventEffect::Heavy) if lends_session => self.begin_apply(),
            Ok(EventEffect::RescanRequired) | Err(SessionError::Scan(_)) if lends_session => {
                let session = self
                    .state
                    .borrow_mut()
                    .session
                    .take()
                    .expect("the gate lends an open session");
                self.begin_scan(session, false);
            }
            Ok(EventEffect::Ignored | EventEffect::Heavy | EventEffect::RescanRequired)
            | Err(SessionError::Scan(_)) => {}
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn begin_apply(&self) {
        {
            let mut state = self.state.borrow_mut();
            let ShellState {
                watch,
                session,
                applying,
                ..
            } = &mut *state;
            let batch = watch
                .as_mut()
                .and_then(|watch| watch.queue.take_heavy())
                .expect("a heavy batch waits in the queue");
            let session = session.take().expect("a heavy batch applies to an open session");
            *applying = Some(ApplyJob::start(session, batch));
        }
        self.window.setSubtitle(&NSString::from_str(UPDATING_INDEX));
    }

    fn finish_apply(&self) -> Option<Result<EventEffect, SessionError>> {
        let settled = {
            let mut state = self.state.borrow_mut();
            let job = state.applying.take_if(|job| job.is_finished())?;
            let (session, applied) = job.finish();
            state.session = Some(session);
            state
                .watch
                .as_mut()
                .expect("a heavy batch comes from the watched volume")
                .queue
                .settle(applied)
        };
        self.window.setSubtitle(&NSString::from_str(""));
        Some(settled)
    }

    fn refresh(&self) {
        let rebuilt = self.state.borrow_mut().rebuild_data(SystemTime::now());
        if let Err(error) = rebuilt {
            return self.fail(&error.to_string());
        }
        {
            let state = self.state.borrow();
            if let (Some(ScreenViews::Main { diagram, .. }), Some(data)) = (state.views.as_ref(), state.data.as_ref()) {
                let tree = data.tree.clone();
                let marked = data.marked.clone();
                let tokens = state.palette.tokens;
                let focus = state.focus;
                let small_groups_always = state.small_groups.always;
                diagram.update_state(|diagram| {
                    let reduce_motion = diagram.reduce_motion;
                    *diagram = DiagramState::new(tree, diagram.mode, DIAGRAM, tokens, diagram.width, diagram.height);
                    diagram.reduce_motion = reduce_motion;
                    diagram.set_small_groups_always(small_groups_always);
                    diagram.jump_to(focus);
                    diagram.jump_marked(marked);
                });
            }
        }
        self.show_current();
        self.refresh_tray();
    }

    fn restyle(&self) {
        let palette = self.palette();
        {
            let mut state = self.state.borrow_mut();
            state.palette = palette;
            if let Some(diagram) = state.views.as_ref().and_then(ScreenViews::diagram) {
                diagram.update_state(|diagram| diagram.palette = palette.tokens);
            }
        }
        self.show_current();
        self.refresh_tray();
    }

    fn show_access(&self, access: AccessState) {
        {
            let mut state = self.state.borrow_mut();
            state.stage = Stage::Access(access);
            state.views = None;
        }
        self.show_current();
    }

    fn show_current(&self) {
        autoreleasepool(|_| self.show_current_views());
        self.refresh_tray_summary();
    }

    fn refresh_tray_summary(&self) {
        let mut state = self.state.borrow_mut();
        let ShellState {
            views: Some(ScreenViews::Main { screen, diagram }),
            session: Some(session),
            data: Some(data),
            focus,
            tray_summary,
            ..
        } = &mut *state
        else {
            return;
        };
        let index = session.index().expect("the main screen shows a scanned volume");
        let items: Vec<PathBuf> = session.tray().items().iter().map(|item| item.path.clone()).collect();
        let fresh = tray_summary
            .as_ref()
            .is_some_and(|cached| cached.generation == index.generation() && cached.items == items);
        if !fresh {
            *tray_summary = Some(CachedTraySummary {
                generation: index.generation(),
                items,
                summary: session.tray_summary(),
            });
            data.mark_tray(index, session.tray());
            screen.list.show(data.level_rows(*focus));
            let marked = data.marked.clone();
            diagram.update_state(|diagram| diagram.set_marked(marked, Instant::now()));
        }
        let summary = tray_summary.as_ref().expect("the summary is cached").summary;
        screen.show_tray_summary(&summary.title());
    }

    fn drag_from_list(&self) {
        self.hide_refusal();
        let mut state = self.state.borrow_mut();
        let Some(ScreenViews::Main { screen, .. }) = &state.views else {
            return;
        };
        let rows = screen.list.source.dragged_rows();
        let data = state.data.as_ref().expect("the main screen shows built data");
        let level = data.level_rows(state.focus);
        let session = state.session.as_ref().expect("a drag reads an open session");
        let index = session.index().expect("the main screen shows a scanned volume");
        let paths: Vec<PathBuf> = rows
            .iter()
            .filter_map(|row| level.get(*row))
            .filter_map(|row| state.indexed_node(row.node))
            .map(|node| index.path(node))
            .collect();
        let verdict = session.judge_drop(&paths);
        screen.arm_tray(verdict.hover());
        let refusal = verdict.refusal_line();
        state.drag = Some(paths);
        state.drag_refusal = refusal;
    }

    fn drop_on_tray(&self) {
        let verdict = {
            let mut state = self.state.borrow_mut();
            let paths = state.drag.clone().expect("a drop follows the start of a drag");
            let session = state.session.as_mut().expect("a drop reads an open session");
            session.drop_into_tray(&paths)
        };
        if let Some(line) = verdict.refusal_line() {
            self.show_refusal(&line);
        }
        self.refresh_tray_summary();
    }

    fn drag_ended(&self) {
        let refusal = {
            let mut state = self.state.borrow_mut();
            state.drag = None;
            let refusal = state.drag_refusal.take();
            let Some(ScreenViews::Main { screen, .. }) = &state.views else {
                return;
            };
            let refused_over_tray = screen.tray.hover_kind() == Some(DropKind::Denied)
                && screen
                    .list
                    .source
                    .drag_end()
                    .is_some_and(|end| !end.accepted && screen.tray.contains_screen_point(end.screen_point));
            screen.disarm_tray();
            refusal.filter(|_| refused_over_tray)
        };
        if let Some(line) = refusal {
            self.show_refusal(&line);
        }
    }

    fn show_refusal(&self, line: &str) {
        let timer = self.once(REFUSAL_SHOWN, ShellController::hide_refusal);
        let mut state = self.state.borrow_mut();
        if let Some(previous) = state.refusal_timer.replace(timer) {
            previous.invalidate();
        }
        if let Some(ScreenViews::Main { screen, .. }) = &state.views {
            screen.show_refusal(Some(line));
        }
    }

    fn hide_refusal(&self) {
        let mut state = self.state.borrow_mut();
        if let Some(timer) = state.refusal_timer.take() {
            timer.invalidate();
        }
        if let Some(ScreenViews::Main { screen, .. }) = &state.views {
            screen.show_refusal(None);
        }
    }

    fn show_current_views(&self) {
        let marker = self.marker;
        let mut state = self.state.borrow_mut();
        state.palette = self.palette();
        let palette = state.palette;
        let previous = state.views.take();
        sync_assistant(&mut state, previous.as_ref());
        let rescan_allowed = state.gate().allows(ShellAction::Rescan);
        if let Some(data) = state.data.as_mut() {
            data.rescan_allowed = rescan_allowed;
        }
        let key = state.screen_key();
        let previous = match previous {
            Some(mut views) if key.is_some() && key == state.views_key => {
                views.update(
                    state.data.as_ref().expect("a keyed screen shows built data"),
                    state.focus,
                );
                self.focus_assistant_input(&views);
                state.views = Some(views);
                return;
            }
            previous => previous,
        };
        let views = match (state.stage, state.section) {
            (Stage::Access(access), _) => {
                let screen = AccessScreen::build(marker, access, &palette, &self.actions);
                self.size_window(screen.size(), false);
                ScreenViews::Access(screen)
            }
            (Stage::Scanning, _) => {
                self.size_window(WINDOW, true);
                let status = state.scan.as_ref().expect("a scan runs while scanning").status.clone();
                let data = ShellData::measured(
                    state.access,
                    state.overview(),
                    None,
                    &Expansion::default(),
                    status,
                    CareModels::default(),
                    state.assistant.models(),
                );
                let screen = ScanningScreen::build(marker, &data, &palette, &self.actions);
                let diagram = match previous {
                    Some(ScreenViews::Scanning { diagram, .. }) => diagram,
                    _ => match self.diagram_view(
                        ScanningScreen::diagram_state(&data, &palette),
                        state.small_groups.always,
                    ) {
                        Ok(diagram) => diagram,
                        Err(error) => {
                            drop(state);
                            return self.fail(&error);
                        }
                    },
                };
                place_diagram(&diagram, &screen.frame.pane, screen.diagram_frame);
                ScreenViews::Scanning { screen, diagram }
            }
            (Stage::Main, Section::Recommendations) => {
                self.size_window(WINDOW, true);
                let data = state.data.as_ref().expect("the main stage has data");
                ScreenViews::Recommendations(RecommendationsScreen::build(marker, data, &palette, &self.actions))
            }
            (Stage::Main, Section::Duplicates) => {
                self.size_window(WINDOW, true);
                let data = state.data.as_ref().expect("the main stage has data");
                ScreenViews::Duplicates(DuplicatesScreen::build(marker, data, &palette, &self.actions))
            }
            (Stage::Main, Section::Assistant) => {
                self.size_window(WINDOW, true);
                let data = state.data.as_ref().expect("the main stage has data");
                ScreenViews::Assistant(AssistantScreen::build(marker, data, &palette, &self.actions))
            }
            (Stage::Main, Section::Model) => {
                self.size_window(WINDOW, true);
                let data = state.data.as_ref().expect("the main stage has data");
                ScreenViews::Model(ModelScreen::build(marker, data, &palette, &self.actions))
            }
            (Stage::Main, Section::Disk) => {
                self.size_window(WINDOW, true);
                let data = state.data.as_ref().expect("the main stage has data");
                let screen = MainScreen::build(marker, data, state.focus, state.mode, &palette, &self.actions);
                let diagram = match previous {
                    Some(ScreenViews::Main { diagram, .. }) => diagram,
                    _ => match self.diagram_view(
                        MainScreen::diagram_state(data, state.focus, state.mode, &palette),
                        state.small_groups.always,
                    ) {
                        Ok(diagram) => diagram,
                        Err(error) => {
                            drop(state);
                            return self.fail(&error);
                        }
                    },
                };
                place_diagram(&diagram, &screen.frame.pane, screen.diagram_frame);
                ScreenViews::Main { screen, diagram }
            }
        };
        self.present_built_views(&mut state, views, key);
    }

    fn present_built_views(&self, state: &mut ShellState, views: ScreenViews, key: Option<ScreenKey>) {
        let content: Retained<NSView> = match &views {
            ScreenViews::Access(screen) => screen.root.clone().into_super(),
            ScreenViews::Scanning { screen, .. } => screen.frame.root(),
            ScreenViews::Main { screen, .. } => screen.frame.root(),
            ScreenViews::Recommendations(screen) => screen.frame.root(),
            ScreenViews::Assistant(screen) => screen.frame.root(),
            ScreenViews::Duplicates(screen) => screen.frame.root(),
            ScreenViews::Model(screen) => screen.frame.root(),
        };
        content.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        self.root.show(&content);
        self.focus_assistant_input(&views);
        state.views_key = key;
        state.views = Some(views);
    }

    fn focus_assistant_input(&self, views: &ScreenViews) {
        let ScreenViews::Assistant(screen) = views else {
            return;
        };
        let Some(text) = screen.input.as_ref().map(|input| &input.text) else {
            return;
        };
        let focused = self
            .window
            .firstResponder()
            .is_some_and(|responder| std::ptr::eq(Retained::as_ptr(&responder).cast(), Retained::as_ptr(text)));
        if text.isEditable() && !focused {
            self.window.makeFirstResponder(Some(text));
        }
    }

    fn diagram_view(
        &self,
        mut diagram: DiagramState,
        small_groups_always: bool,
    ) -> Result<Retained<DiagramView>, String> {
        diagram.reduce_motion = reduce_motion();
        diagram.set_small_groups_always(small_groups_always);
        let renderer = DiagramRenderer::system_default(&diagram_metallib()).map_err(|error| error.to_string())?;
        let frame = NSRect::new(
            NSPoint::ZERO,
            NSSize::new(f64::from(diagram.width), f64::from(diagram.height)),
        );
        let view = DiagramView::new(
            self.marker,
            frame,
            diagram,
            DiagramGraphics {
                renderer,
                atlas: GlyphAtlas::default(),
            },
        );
        let observer = self.this.clone();
        view.observe(DiagramObserver::new(move |change| {
            if let Some(controller) = observer.upgrade() {
                controller.diagram_changed(change);
            }
        }));
        Ok(view)
    }

    fn diagram_changed(&self, change: DiagramChange) {
        let tip = {
            let state = self.state.borrow();
            change.hovered.and_then(|node| state.hover_tip(node))
        };
        match (tip, change.pointer) {
            (Some(tip), Some(pointer)) => {
                let palette = self.state.borrow().palette;
                self.hover.show(&tip, &palette, &self.window, pointer);
            }
            _ => self.hover.hide(),
        }
        let focus_changed = {
            let state = self.state.borrow();
            state.stage == Stage::Main && state.section == Section::Disk && state.focus != change.focus
        };
        if focus_changed {
            self.focus_on(change.focus);
        }
    }

    fn focus_on(&self, node: usize) {
        let needs_expansion = {
            let mut state = self.state.borrow_mut();
            state.remember_focus(node);
            let origin = state.data.as_ref().map(|data| data.tree.node(node).origin);
            match (origin, state.session.as_ref().and_then(Session::index)) {
                (Some(NodeOrigin::Indexed(id)), Some(index)) => !state.expansion.shows_rings_below(index, id),
                _ => false,
            }
        };
        if needs_expansion {
            return self.refresh();
        }
        if let Some(diagram) = self.state.borrow().views.as_ref().and_then(ScreenViews::diagram) {
            diagram.update_state(|diagram| diagram.set_focus(node, Instant::now()));
        }
        self.show_current();
    }

    fn open_settings(&self) {
        self.state.borrow_mut().settings_opened = true;
        let url = NSURL::URLWithString(&NSString::from_str(FDA_SETTINGS_URL)).expect("the settings URL is valid");
        NSWorkspace::sharedWorkspace().openURL(&url);
    }

    fn change_mode(&self, mode: DiagramMode) {
        remember_mode(mode);
        {
            let mut state = self.state.borrow_mut();
            state.mode = mode;
            if let Some(diagram) = state.views.as_ref().and_then(ScreenViews::diagram) {
                diagram.update_state(|diagram| diagram.set_mode(mode));
            }
        }
        self.show_current();
    }

    fn navigate(&self, section: Section) {
        let allowed = {
            let state = self.state.borrow();
            let care_ready = state.data.as_ref().is_some_and(|data| data.care_ready);
            state.stage == Stage::Main
                && state.section != section
                && Navigation {
                    current: state.section,
                    care_ready,
                }
                .allows(section)
        };
        if allowed {
            self.hover.hide();
            self.state.borrow_mut().section = section;
            self.show_current();
        }
    }

    fn toggle_card(&self, position: usize) {
        let mut state = self.state.borrow_mut();
        let model = &mut state.care_mut().recommendations;
        let Some(card) = model.cards.get_mut(position).filter(|card| card.selectable()) else {
            return;
        };
        if card.rows.is_empty() {
            card.selected = !card.selected;
        } else {
            card.toggle_rows();
        }
        if let Some(ScreenViews::Recommendations(screen)) = &state.views {
            screen.show_selection(&state.care().recommendations);
        }
    }

    fn change_projects(&self, change: impl FnOnce(&mut CardModel)) {
        let mut state = self.state.borrow_mut();
        let model = &mut state.care_mut().recommendations;
        let Some(card) = model
            .cards
            .iter_mut()
            .find(|card| card.provenance == Provenance::Projects)
        else {
            return;
        };
        change(card);
        if let Some(ScreenViews::Recommendations(screen)) = &state.views {
            screen.show_selection(&state.care().recommendations);
        }
    }

    fn send_cards_to_tray(&self) {
        {
            let mut state = self.state.borrow_mut();
            let targets: Vec<_> = state
                .care()
                .recommendations
                .cards
                .iter()
                .flat_map(CardModel::selected_targets)
                .collect();
            let session = state
                .session
                .as_mut()
                .expect("recommendations come from an open session");
            for target in &targets {
                session.add_target_to_tray(target);
            }
        }
        self.open_tray();
    }

    fn toggle_duplicate_search(&self) {
        if let Some(job) = &self.state.borrow().search {
            job.stop();
            return;
        }
        let has_results = self.state.borrow().duplicate_summary.is_some();
        if has_results {
            {
                let mut state = self.state.borrow_mut();
                state.duplicate_summary = None;
                state.clone_mode = false;
            }
            return self.refresh();
        }
        let session = self.state.borrow_mut().session.take();
        let Some(session) = session else {
            return;
        };
        let timer = self.repeating(SEARCH_TICK, ShellController::search_tick);
        {
            let mut state = self.state.borrow_mut();
            let job = DuplicateJob::start(session, timer);
            state.care_mut().duplicates = DuplicatesModel {
                state: DuplicatesState::Searching {
                    stage: job.stage,
                    done: job.done,
                    total: job.total,
                },
                clone_mode: false,
            };
            state.search = Some(job);
        }
        self.show_current();
    }

    fn search_tick(&self) {
        let finished = {
            let mut state = self.state.borrow_mut();
            let ShellState { search, views, .. } = &mut *state;
            let Some(job) = search.as_mut() else {
                return;
            };
            if job.absorb_progress()
                && let Some(ScreenViews::Duplicates(screen)) = views.as_ref()
            {
                screen.show_progress(job.stage.title(), job.done, job.total);
            }
            if job.is_finished() { search.take() } else { None }
        };
        if let Some(job) = finished {
            let summary = DuplicateSummary {
                candidates: job.candidates,
                seconds: job.seconds(),
            };
            let (session, outcome) = job.finish();
            {
                let mut state = self.state.borrow_mut();
                state.session = Some(session);
                state.duplicate_summary = outcome.is_ok().then_some(summary);
            }
            match outcome {
                Ok(()) | Err(SearchError::Cancelled) => self.refresh(),
                Err(error) => self.fail(&error.to_string()),
            }
        }
    }

    fn select_duplicate_action(&self, clone: bool) {
        self.state.borrow_mut().clone_mode = clone;
        self.refresh();
    }

    fn toggle_copy(&self, tag: usize) {
        let mut state = self.state.borrow_mut();
        let duplicates = &mut state.care_mut().duplicates;
        let clone_mode = duplicates.clone_mode;
        let (group, copy) = DuplicatesLayout::of(duplicates.groups()).toggles[tag];
        let member = &mut duplicates.groups_mut()[group].copies[copy];
        if member.can_check(clone_mode) {
            member.checked = !member.checked;
        }
        if let Some(ScreenViews::Duplicates(screen)) = &state.views {
            screen.show_selection(&state.care().duplicates);
        }
    }

    fn apply_duplicates(&self) {
        let (clone_mode, pairs) = {
            let state = self.state.borrow();
            let duplicates = &state.care().duplicates;
            (duplicates.clone_mode, duplicates.checked_copies())
        };
        if !clone_mode {
            {
                let mut state = self.state.borrow_mut();
                let session = state.session.as_mut().expect("duplicates come from an open session");
                for (kept, copy) in &pairs {
                    session.add_duplicate_copy_to_tray(*copy, *kept);
                }
            }
            return self.open_tray();
        }
        let mut freed = 0;
        let mut failures = Vec::new();
        {
            let mut state = self.state.borrow_mut();
            let session = state.session.as_mut().expect("duplicates come from an open session");
            for (keep, copy) in pairs {
                let index = session.index().expect("duplicates come from an open session");
                let path = index.path(copy);
                let reclaimed = index.reclaimable(copy).map_or(0, |reclaimable| reclaimable.bytes);
                match session.replace_with_clone(keep, copy) {
                    Ok(_) => freed += reclaimed,
                    Err(error) => failures.push(format!("{} — {}", tilde(&path), clone_refusal_text(&error))),
                }
            }
        }
        self.refresh();
        let mut lines = vec![format!("{CLONED} {}", format_size(freed))];
        if !failures.is_empty() {
            lines.push(NOT_CLONED.to_owned());
            lines.extend(failures);
        }
        self.inform(&lines.join("\n"));
    }

    fn open_tray(&self) {
        let model = self.measured_tray();
        self.show_tray(&model, false);
    }

    fn remove_application(&self, node: usize) {
        let added = {
            let mut state = self.state.borrow_mut();
            let Some(bundle) = state.indexed_node(node) else {
                return;
            };
            let Some(session) = state.session.as_mut() else {
                return;
            };
            session.add_application_to_tray(bundle)
        };
        if added {
            self.open_tray();
        }
    }

    fn measured_tray(&self) -> TrayModel {
        let (items, estimate) = {
            let state = self.state.borrow();
            let session = state.session.as_ref().expect("the tray belongs to an open session");
            let items: Vec<PathBuf> = session
                .tray()
                .items()
                .iter()
                .map(|item| item.physical.clone())
                .collect();
            let fresh = state
                .tray_measurement
                .as_ref()
                .is_some_and(|measurement| measurement.items == items);
            let index = session.index().expect("the tray belongs to a scanned volume");
            (
                items,
                (!fresh).then(|| TrayModel::estimated_sizes(index, session.tray())),
            )
        };
        if let Some(estimate) = estimate {
            let timer = self.repeating(MEASURE_TICK, ShellController::measure_tick);
            self.state.borrow_mut().tray_measurement = Some(TrayMeasurement::start(items, estimate, timer));
        }
        let state = self.state.borrow();
        let ShellState {
            session,
            tray_measurement,
            ..
        } = &*state;
        let session = session.as_ref().expect("the tray belongs to an open session");
        let index = session.index().expect("the tray belongs to a scanned volume");
        let measurement = tray_measurement.as_ref().expect("the tray is being measured");
        match measurement.exact() {
            Some(exact) => TrayModel::measured(index, session.tray(), exact),
            None => {
                TrayModel::measured(index, session.tray(), measurement.estimate()).with_progress(measurement.progress())
            }
        }
    }

    fn measure_tick(&self) {
        let advanced = self
            .state
            .borrow_mut()
            .tray_measurement
            .as_mut()
            .map(TrayMeasurement::advance);
        match advanced {
            Some(Ok(())) => self.refresh_tray(),
            Some(Err(error)) => self.fail(&error.to_string()),
            None => {}
        }
    }

    fn show_tray(&self, model: &TrayModel, stitch: bool) {
        let palette = self.state.borrow().palette;
        let screen = TrayScreen::build(
            self.marker,
            model,
            &palette,
            &self.actions,
            self.window.contentLayoutRect().size.height,
        );
        let existing = self.state.borrow_mut().tray.take();
        let window = if let Some(sheet) = existing {
            screen.keep_place_of(&sheet.screen);
            sheet.window
        } else {
            self.begin_sheet(screen.size())
        };
        window.setContentSize(screen.size());
        window.setContentView(Some(&screen.root));
        if stitch && let Some(ornament) = &screen.ornament {
            ornament.stitch_once(reduce_motion());
        }
        self.state.borrow_mut().tray = Some(TraySheet {
            window,
            screen,
            model: model.clone(),
        });
    }

    fn begin_sheet(&self, size: NSSize) -> Retained<NSWindow> {
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(self.marker),
                NSRect::new(NSPoint::ZERO, size),
                NSWindowStyleMask::Titled,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        self.window.beginSheet_completionHandler(&window, None);
        window
    }

    fn refresh_tray(&self) {
        let state = self.state.borrow();
        let confirming =
            state.delete.is_none() && state.tray.as_ref().is_some_and(|sheet| sheet.screen.ornament.is_none());
        drop(state);
        if confirming {
            self.open_tray();
        }
    }

    fn delete_tray(&self) {
        if self.state.borrow().delete.is_some() {
            return;
        }
        let count = {
            let state = self.state.borrow();
            let session = state.session.as_ref().expect("the tray belongs to an open session");
            session.tray().items().len()
        };
        if count == 0 || !self.confirm_delete(count) {
            return;
        }
        let taken = {
            let mut state = self.state.borrow_mut();
            let sizes = state.tray_measurement.take().map(|measurement| {
                measurement
                    .exact()
                    .map_or_else(|| measurement.estimate().to_vec(), <[Reclaimable]>::to_vec)
            });
            let session = state.session.as_mut().expect("the tray belongs to an open session");
            let sizes = sizes.unwrap_or_else(|| {
                let index = session.index().expect("the tray belongs to a scanned volume");
                TrayModel::estimated_sizes(index, session.tray())
            });
            session.take_tray_for_delete(&sizes)
        };
        let (subjects, refusals) = match taken {
            Ok(taken) => taken,
            Err(error) => return self.inform(&format!("{PROTECTION_UNCHECKED} {error}")),
        };
        let timer = self.repeating(DELETE_TICK, ShellController::delete_tick);
        self.state.borrow_mut().delete = Some(DeleteRun::start(DeleteJob::start(subjects), refusals, timer));
        self.show_deleting(DeleteStatus::default());
    }

    fn confirm_delete(&self, count: usize) -> bool {
        let alert = NSAlert::new(self.marker);
        alert.setMessageText(&NSString::from_str(&format!(
            "{DELETE_CONFIRM} {}?",
            format_count(count as u64, &OBJECTS)
        )));
        alert.setInformativeText(&NSString::from_str(DELETE_CONFIRM_HINT));
        alert
            .addButtonWithTitle(&NSString::from_str(DELETE))
            .setHasDestructiveAction(true);
        alert
            .addButtonWithTitle(&NSString::from_str(CANCEL))
            .setKeyEquivalent(&NSString::from_str(ESCAPE));
        alert.runModal() == NSAlertFirstButtonReturn
    }

    fn delete_tick(&self) {
        let (status, finished) = {
            let state = self.state.borrow();
            let Some(run) = state.delete.as_ref() else {
                return;
            };
            (run.status(), run.is_finished())
        };
        if !finished {
            return self.show_deleting(status);
        }
        let run = self
            .state
            .borrow_mut()
            .delete
            .take()
            .expect("a finished delete is running");
        let (report, refusals) = run.finish();
        let outcome = DeleteOutcome::from_report(&report, &refusals);
        let settled = self
            .state
            .borrow_mut()
            .session
            .as_mut()
            .expect("the deleted items belong to an open session")
            .settle_delete(&report);
        if let Err(error) = settled {
            return self.fail(&error.to_string());
        }
        match DiskOverview::measure(Path::new(SYSTEM_DATA_ROOT)) {
            Ok(overview) => self.state.borrow_mut().overview = Some(overview),
            Err(error) => return self.fail(&error.to_string()),
        }
        self.refresh();
        let model = {
            let state = self.state.borrow();
            let Some(sheet) = state.tray.as_ref() else {
                return;
            };
            sheet.model.clone().with_deleted(outcome)
        };
        self.show_tray(&model, true);
    }

    fn show_deleting(&self, status: DeleteStatus) {
        let model = {
            let state = self.state.borrow();
            let Some(sheet) = state.tray.as_ref() else {
                return;
            };
            sheet.model.clone().with_deleting(status)
        };
        self.show_tray(&model, false);
    }

    fn unmark_tray_row(&self, position: usize) {
        {
            let mut state = self.state.borrow_mut();
            let nodes = state
                .tray
                .as_ref()
                .and_then(|sheet| sheet.model.row_nodes.get(position))
                .cloned()
                .expect("an unmark button stands on a shown row");
            let session = state.session.as_mut().expect("the tray belongs to an open session");
            for node in nodes {
                session.remove_from_tray(node);
            }
        }
        let model = self.measured_tray();
        self.show_tray(&model, false);
        self.refresh_tray_summary();
    }

    fn close_tray(&self) {
        let sheet = {
            let mut state = self.state.borrow_mut();
            state.tray_measurement = None;
            state.tray.take()
        };
        if let Some(sheet) = sheet {
            self.window.endSheet(&sheet.window);
            sheet.window.orderOut(None);
        }
        self.show_current();
    }

    fn purge_space(&self) {
        let model = {
            let state = self.state.borrow();
            if state.purge.is_some() {
                return;
            }
            let overview = state.overview();
            PurgeModel::Confirming {
                estimate: overview.volumes.purgeable.value,
                time_machine: overview.time_machine.explains_purgeable(),
            }
        };
        self.show_purge(model, false);
    }

    fn show_purge(&self, model: PurgeModel, stitch: bool) {
        let palette = self.state.borrow().palette;
        let screen = PurgeScreen::build(
            self.marker,
            &model,
            &palette,
            &self.actions,
            self.window.contentLayoutRect().size.height,
        );
        let existing = self.state.borrow_mut().purge.take();
        let (window, run) = match existing {
            Some(sheet) => (sheet.window, sheet.run),
            None => (self.begin_sheet(screen.size()), None),
        };
        window.setContentSize(screen.size());
        window.setContentView(Some(&screen.root));
        if stitch && let Some(ornament) = &screen.ornament {
            ornament.stitch_once(reduce_motion());
        }
        self.state.borrow_mut().purge = Some(PurgeSheet { window, model, run });
    }

    fn release_purge(&self) {
        let (recovery, estimate) = {
            let state = self.state.borrow();
            let sheet = state
                .purge
                .as_ref()
                .expect("the release button belongs to the open sheet");
            let recovery = state
                .recovery
                .expect("the sheet opens only with the recovery functions");
            (recovery, sheet.model.estimate())
        };
        let timer = self.repeating(PURGE_TICK, ShellController::purge_tick);
        let job = PurgeJob::start(
            recovery,
            PathBuf::from(SYSTEM_DATA_ROOT),
            available_capacity,
            PURGE_TIMEOUT,
        );
        {
            let mut state = self.state.borrow_mut();
            let sheet = state
                .purge
                .as_mut()
                .expect("the release button belongs to the open sheet");
            sheet.run = Some(PurgeRun::start(job, timer));
        }
        self.show_purge(PurgeModel::Releasing(PurgeProgress { estimate, freed: 0 }), false);
    }

    fn purge_tick(&self) {
        let watched = {
            let state = self.state.borrow();
            state
                .purge
                .as_ref()
                .and_then(|sheet| sheet.run.as_ref())
                .map(|run| (run.progress(), run.is_finished()))
        };
        let Some((progress, finished)) = watched else {
            return;
        };
        if !finished {
            return self.show_purge(PurgeModel::Releasing(progress), false);
        }
        let run = self
            .state
            .borrow_mut()
            .purge
            .as_mut()
            .and_then(|sheet| sheet.run.take())
            .expect("a finished release is running");
        let outcome = run.finish().map_err(|error| error.to_string());
        match DiskOverview::measure(Path::new(SYSTEM_DATA_ROOT)) {
            Ok(overview) => self.state.borrow_mut().overview = Some(overview),
            Err(error) => return self.fail(&error.to_string()),
        }
        self.refresh();
        self.show_purge(PurgeModel::Released(outcome), true);
    }

    fn stop_scan(&self) {
        if let Some(job) = &self.state.borrow().scan {
            job.stop();
        }
    }

    fn stop_delete(&self) {
        if let Some(run) = &self.state.borrow().delete {
            run.stop();
        }
    }

    fn stop_purge(&self) {
        if let Some(run) = self.state.borrow().purge.as_ref().and_then(|sheet| sheet.run.as_ref()) {
            run.stop();
        }
    }

    fn close_purge(&self) {
        let sheet = self.state.borrow_mut().purge.take();
        if let Some(sheet) = sheet {
            self.window.endSheet(&sheet.window);
            sheet.window.orderOut(None);
        }
        self.show_current();
    }

    fn show_in_finder(&self, node: usize) {
        let path = self.state.borrow().node_path(node);
        if let Some(path) = path {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&[url]));
        }
    }

    fn clicked_row(&self) -> Option<usize> {
        let state = self.state.borrow();
        let Some(ScreenViews::Main { screen, .. }) = &state.views else {
            return None;
        };
        let row = usize::try_from(screen.list.table.clickedRow()).ok()?;
        let focus = state.focus;
        state.data.as_ref()?.level_rows(focus).get(row).map(|row| row.node)
    }

    fn open_in_terminal(&self, node: usize) {
        let Some(path) = self.state.borrow().node_path(node) else {
            return;
        };
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().map_or_else(|| path.clone(), Path::to_path_buf)
        };
        let workspace = NSWorkspace::sharedWorkspace();
        let Some(terminal) = workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(TERMINAL_BUNDLE))
        else {
            return self.inform(TERMINAL_MISSING);
        };
        let url = NSURL::fileURLWithPath(&NSString::from_str(&directory.to_string_lossy()));
        workspace.openURLs_withApplicationAtURL_configuration_completionHandler(
            &NSArray::from_retained_slice(&[url]),
            &terminal,
            &NSWorkspaceOpenConfiguration::configuration(),
            None,
        );
    }

    fn show_properties(&self, node: usize, anchor: Option<(Retained<NSView>, NSRect)>) {
        let facts = {
            let state = self.state.borrow();
            let index = state.session.as_ref().and_then(Session::index);
            state
                .data
                .as_ref()
                .and_then(|data| NodeFacts::for_node(data, index, node, SystemTime::now()))
        };
        let Some(facts) = facts else {
            return;
        };
        let (view, rect) = anchor.unwrap_or_else(|| {
            let root: Retained<NSView> = self.root.clone().into_super();
            let bounds = root.bounds();
            (root, bounds)
        });
        self.hover.hide();
        let palette = self.state.borrow().palette;
        self.properties
            .show(self.marker, &facts, &palette, &self.actions, &view, rect);
    }

    fn clicked_row_anchor(&self) -> Option<(usize, (Retained<NSView>, NSRect))> {
        let node = self.clicked_row()?;
        let state = self.state.borrow();
        let Some(ScreenViews::Main { screen, .. }) = &state.views else {
            return None;
        };
        let table = &screen.list.table;
        let rect = table.rectOfRow(table.clickedRow());
        let view: Retained<NSView> = table.clone().into_super().into_super();
        Some((node, (view, rect)))
    }

    fn crumb_anchor(&self, node: usize) -> Option<(Retained<NSView>, NSRect)> {
        let state = self.state.borrow();
        let Some(ScreenViews::Main { screen, .. }) = &state.views else {
            return None;
        };
        let tag = NSInteger::try_from(node).ok()?;
        let button = screen.crumbs.buttons().into_iter().find(|button| button.tag() == tag)?;
        let bounds = button.bounds();
        Some((button.into_super().into_super(), bounds))
    }

    fn toggle_preview(&self) {
        let targets = self.preview_targets();
        self.preview.set_targets(&targets);
        let Some(panel) = (unsafe { QLPreviewPanel::sharedPreviewPanel(self.marker) }) else {
            return;
        };
        if panel.isVisible() {
            if targets.is_empty() {
                panel.orderOut(None);
            } else {
                unsafe { panel.reloadData() };
            }
        } else if !targets.is_empty() {
            panel.makeKeyAndOrderFront(None);
        }
    }

    fn preview_targets(&self) -> PreviewTargets {
        let state = self.state.borrow();
        let Some(index) = state.session.as_ref().and_then(Session::index) else {
            return PreviewTargets::default();
        };
        match (&state.views, &state.data) {
            (Some(ScreenViews::Main { screen, .. }), Some(data)) => {
                let rows = data.level_rows(state.focus);
                let nodes: Vec<usize> = selected_rows(&screen.list.table.selectedRowIndexes())
                    .filter_map(|row| rows.get(row).map(|row| row.node))
                    .collect();
                PreviewTargets::from_level(&data.tree, index, &nodes)
            }
            (Some(ScreenViews::Duplicates(screen)), Some(data)) => {
                let Some((outline, _)) = &screen.outline else {
                    return PreviewTargets::default();
                };
                let keys: Vec<DuplicateKey> = selected_rows(&outline.selectedRowIndexes())
                    .filter_map(|row| outline.itemAtRow(NSInteger::try_from(row).ok()?))
                    .filter_map(|item| {
                        item.downcast_ref::<NSString>()
                            .map(|key| DuplicateKey::parse(&key.to_string()))
                    })
                    .collect();
                PreviewTargets::from_duplicates(data.care.duplicates.groups(), &keys)
            }
            _ => PreviewTargets::default(),
        }
    }

    fn copy_tmutil(&self) {
        let commands = tmutil_commands(&self.state.borrow().overview().volumes.snapshots);
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        pasteboard.setString_forType(&NSString::from_str(&commands), unsafe { NSPasteboardTypeString });
        self.inform(TMUTIL_COPIED);
    }

    fn export_csv(&self) {
        let volume = self
            .state
            .borrow()
            .data
            .as_ref()
            .map(|data| data.disk_name.clone())
            .unwrap_or_default();
        let panel = NSSavePanel::savePanel(self.marker);
        panel.setNameFieldStringValue(&NSString::from_str(&csv_file_name(&volume)));
        panel.setCanCreateDirectories(true);
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
        let written = {
            let state = self.state.borrow();
            match state.session.as_ref().and_then(Session::index) {
                Some(index) => fs::write(&path, csv_lines(index, index.root())),
                None => return,
            }
        };
        if let Err(error) = written {
            self.fail(&error.to_string());
        }
    }

    fn relaunch(&self) {
        let bundle = NSBundle::mainBundle();
        let spawned = match bundle.bundleIdentifier() {
            Some(identifier) => Command::new(OPEN).args(["-n", "-b", &identifier.to_string()]).spawn(),
            None => std::env::current_exe().and_then(|executable| Command::new(executable).spawn()),
        };
        match spawned {
            Ok(_) => NSApplication::sharedApplication(self.marker).terminate(None),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn fail_unresponsive(&self, path: &Path) {
        let alert = NSAlert::new(self.marker);
        alert.setMessageText(&NSString::from_str(UNRESPONSIVE_TITLE));
        alert.setInformativeText(&NSString::from_str(&unresponsive_body(&path.to_string_lossy())));
        alert.addButtonWithTitle(&NSString::from_str(OPEN_DISK_UTILITY));
        alert.addButtonWithTitle(&NSString::from_str(FAILURE_CLOSE));
        if alert.runModal() == NSAlertFirstButtonReturn {
            open_disk_utility();
        }
    }

    fn inform(&self, message: &str) {
        let alert = NSAlert::new(self.marker);
        alert.setMessageText(&NSString::from_str(message));
        alert.addButtonWithTitle(&NSString::from_str(FAILURE_CLOSE));
        alert.runModal();
    }

    fn fail(&self, reason: &str) {
        let alert = NSAlert::new(self.marker);
        alert.setMessageText(&NSString::from_str(FAILURE_TITLE));
        alert.setInformativeText(&NSString::from_str(reason));
        alert.addButtonWithTitle(&NSString::from_str(FAILURE_CLOSE));
        alert.runModal();
        NSApplication::sharedApplication(self.marker).terminate(None);
    }

    fn palette(&self) -> ShellPalette {
        let candidates = unsafe { [NSAppearanceNameAqua, NSAppearanceNameDarkAqua] };
        let names = NSArray::from_slice(&candidates);
        let dark = self
            .window
            .effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names)
            .is_some_and(|name| &*name == unsafe { NSAppearanceNameDarkAqua });
        ShellPalette {
            tokens: if dark { &Palette::DARK } else { &Palette::LIGHT },
            high_contrast: NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast(),
        }
    }

    fn size_window(&self, size: NSSize, resizable: bool) {
        let mut style = self.window.styleMask();
        style.set(NSWindowStyleMask::Resizable, resizable);
        self.window.setStyleMask(style);
        self.window.setContentMinSize(size);
        let current = self.window.contentLayoutRect().size;
        let fitted = if resizable {
            NSSize::new(current.width.max(size.width), current.height.max(size.height))
        } else {
            size
        };
        if fitted != current {
            self.window.setContentSize(fitted);
        }
    }

    fn observe_contrast(&self) {
        let observer = self.this.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            if let Some(controller) = observer.upgrade() {
                controller.restyle();
            }
        });
        let token = unsafe {
            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserverForName_object_queue_usingBlock(
                    Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &block,
                )
        };
        self.state.borrow_mut().contrast_observer = Some(token);
    }

    fn repeating(&self, interval: f64, tick: fn(&ShellController)) -> Retained<NSTimer> {
        self.timer(interval, true, tick)
    }

    fn once(&self, delay: Duration, tick: fn(&ShellController)) -> Retained<NSTimer> {
        self.timer(delay.as_secs_f64(), false, tick)
    }

    fn timer(&self, interval: f64, repeats: bool, tick: fn(&ShellController)) -> Retained<NSTimer> {
        let controller = self.this.clone();
        let block = RcBlock::new(move |_: NonNull<NSTimer>| {
            if let Some(controller) = controller.upgrade() {
                tick(&controller);
            }
        });
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, repeats, &block) }
    }
}

impl ActionHandler for ShellController {
    fn perform(&self, action: ShellAction) {
        match action {
            ShellAction::OpenSettings => self.open_settings(),
            ShellAction::ScanWithoutAccess => {
                self.state.borrow_mut().scan_without_access = true;
                let session = self.state.borrow_mut().session.take();
                match session {
                    Some(session) => self.begin_scan(session, false),
                    None => self.open_session(),
                }
            }
            ShellAction::Relaunch => self.relaunch(),
            ShellAction::Rescan => self.rescan(),
            ShellAction::StopScan => self.stop_scan(),
            ShellAction::ChangeMode(mode) => self.change_mode(mode),
            ShellAction::ToggleSmallGroups => self.toggle_small_groups(),
            ShellAction::Focus(node) => self.focus_on(node),
            ShellAction::ShowInFinder(node) => self.show_in_finder(node),
            ShellAction::ShowClickedRowInFinder => {
                if let Some(node) = self.clicked_row() {
                    self.show_in_finder(node);
                }
            }
            ShellAction::OpenInTerminal(node) => self.open_in_terminal(node),
            ShellAction::OpenClickedRowInTerminal => {
                if let Some(node) = self.clicked_row() {
                    self.open_in_terminal(node);
                }
            }
            ShellAction::ShowProperties(node) => self.show_properties(node, self.crumb_anchor(node)),
            ShellAction::ShowClickedRowProperties => {
                if let Some((node, anchor)) = self.clicked_row_anchor() {
                    self.show_properties(node, Some(anchor));
                }
            }
            ShellAction::CopyTmutil => self.copy_tmutil(),
            ShellAction::RemoveApplication(node) => self.remove_application(node),
            ShellAction::RemoveClickedApplication => {
                if let Some(node) = self.clicked_row() {
                    self.remove_application(node);
                }
            }
            ShellAction::ExportCsv => self.export_csv(),
            ShellAction::TogglePreview => self.toggle_preview(),
            ShellAction::OpenClickedRow => {
                let openable = self.clicked_row().filter(|node| {
                    let state = self.state.borrow();
                    state
                        .data
                        .as_ref()
                        .is_some_and(|data| !data.tree.node(*node).children.is_empty())
                });
                if let Some(node) = openable {
                    self.focus_on(node);
                }
            }
            ShellAction::Navigate(section) => self.navigate(section),
            ShellAction::OpenTray => self.open_tray(),
            ShellAction::ToggleCard(position) => self.toggle_card(position),
            ShellAction::ToggleProjectRow(position) => self.change_projects(|card| card.toggle_row(position)),
            ShellAction::ToggleProjectGroup(group) => {
                self.change_projects(|card| card.toggle_group(group == ProjectRow::STALE_GROUP));
            }
            ShellAction::SendCardsToTray => self.send_cards_to_tray(),
            ShellAction::ToggleDuplicateSearch => self.toggle_duplicate_search(),
            ShellAction::SelectDuplicateAction(clone) => self.select_duplicate_action(clone),
            ShellAction::ToggleCopy(tag) => self.toggle_copy(tag),
            ShellAction::ApplyDuplicates => self.apply_duplicates(),
            ShellAction::DeleteTray => self.delete_tray(),
            ShellAction::StopDelete => self.stop_delete(),
            ShellAction::CloseTray => self.close_tray(),
            ShellAction::UnmarkTrayRow(position) => self.unmark_tray_row(position),
            ShellAction::DragFromList => self.drag_from_list(),
            ShellAction::DragEnded => self.drag_ended(),
            ShellAction::DropOnTray => self.drop_on_tray(),
            ShellAction::PurgeSpace => self.purge_space(),
            ShellAction::ReleasePurge => self.release_purge(),
            ShellAction::StopPurge => self.stop_purge(),
            ShellAction::ClosePurge => self.close_purge(),
            ShellAction::AskAssistant => self.ask_assistant(),
            ShellAction::AskAboutCard(position) => self.ask_about_card(position),
            ShellAction::StopAnswer => self.stop_answer(),
            ShellAction::AddAnswerToTray(position) => self.add_answer_to_tray(position),
            ShellAction::ToggleAssistant => self.toggle_assistant(),
            #[cfg(feature = "claude-code")]
            ShellAction::ToggleClaudeCode => self.toggle_claude_code(),
            ShellAction::DownloadModel(position) => self.download_model(position),
            ShellAction::PauseDownload(position) => self.pause_download(position),
            ShellAction::CancelDownload(position) => self.cancel_download(position),
            ShellAction::RemoveModel(position) => self.remove_model(position),
            ShellAction::ActivateModel(position) => self.activate_model(position),
            ShellAction::ChooseUnloadDelay => self.choose_unload_delay(),
            ShellAction::SelectUnloadDelay(position) => self.select_unload_delay(position),
            ShellAction::ChangeSource => self.change_source(),
            ShellAction::ChooseModelFile => self.choose_model_file(),
            ShellAction::DeleteOrphan(position) => self.delete_orphan(position),
        }
    }

    fn allows(&self, action: ShellAction) -> bool {
        match action {
            ShellAction::ShowInFinder(node) | ShellAction::OpenInTerminal(node) => {
                self.state.borrow().node_path(node).is_some()
            }
            ShellAction::ShowProperties(node) => self.state.borrow().hover_tip(node).is_some(),
            ShellAction::ShowClickedRowInFinder | ShellAction::OpenClickedRowInTerminal => self
                .clicked_row()
                .is_some_and(|node| self.state.borrow().node_path(node).is_some()),
            ShellAction::ShowClickedRowProperties => self
                .clicked_row()
                .is_some_and(|node| self.state.borrow().hover_tip(node).is_some()),
            ShellAction::CopyTmutil => self
                .clicked_row()
                .is_some_and(|node| self.state.borrow().offers_tmutil(node)),
            ShellAction::PurgeSpace => self
                .clicked_row()
                .is_some_and(|node| self.state.borrow().offers_purge(node)),
            ShellAction::RemoveApplication(node) => {
                let state = self.state.borrow();
                state.gate().allows(action) && state.offers_application_removal(node)
            }
            ShellAction::RemoveClickedApplication => self.clicked_row().is_some_and(|node| {
                let state = self.state.borrow();
                state.gate().allows(action) && state.offers_application_removal(node)
            }),
            _ => self.state.borrow().gate().allows(action),
        }
    }
}

fn open_disk_utility() {
    let workspace = NSWorkspace::sharedWorkspace();
    if let Some(url) = workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(DISK_UTILITY_BUNDLE)) {
        workspace.openApplicationAtURL_configuration_completionHandler(
            &url,
            &NSWorkspaceOpenConfiguration::configuration(),
            None,
        );
    }
}

fn sync_assistant(state: &mut ShellState, previous: Option<&ScreenViews>) {
    if let Some(ScreenViews::Assistant(screen)) = previous {
        state.assistant.chat.draft = screen.draft();
    }
    let ai = state.assistant.models();
    if let Some(data) = state.data.as_mut() {
        data.ai = ai;
    }
}

fn place_diagram(diagram: &DiagramView, pane: &NSView, frame: NSRect) {
    diagram.removeFromSuperview();
    diagram.setFrame(frame);
    diagram.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinYMargin);
    pane.addSubview(diagram);
}
