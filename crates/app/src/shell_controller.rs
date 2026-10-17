use std::cell::RefCell;
use std::path::Path;
use std::process::Command;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::time::{Instant, SystemTime};

use block2::RcBlock;
use domovoi_dups::SearchError;
use domovoi_render::{
    DiagramChange, DiagramGraphics, DiagramObserver, DiagramRenderer, DiagramState, DiagramView, GlyphAtlas, NodeOrigin,
};
use domovoi_rules::{RestoreError, open_trash};
use domovoi_scanner::ScanError;
use domovoi_theme::{Palette, format_size, reduce_motion};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    NSAutoresizingMaskOptions, NSBackingStoreType, NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
};
use objc2_foundation::{
    NSArray, NSBundle, NSNotification, NSOperationQueue, NSPoint, NSRect, NSSize, NSString, NSTimer, NSURL,
};

use crate::access_screen::AccessScreen;
use crate::access_state::AccessState;
use crate::action_handler::ActionHandler;
use crate::bundle_resources::diagram_metallib;
use crate::cache_opening::CacheOpening;
use crate::care_models::CareModels;
use crate::clone_refusal_text::clone_refusal_text;
use crate::diagram_preference::{remember_mode, remembered_mode};
use crate::disk_overview::{DiskOverview, SYSTEM_DATA_ROOT};
use crate::duplicate_job::DuplicateJob;
use crate::duplicate_summary::DuplicateSummary;
use crate::duplicates_layout::DuplicatesLayout;
use crate::duplicates_model::DuplicatesModel;
use crate::duplicates_screen::DuplicatesScreen;
use crate::duplicates_state::DuplicatesState;
use crate::event_effect::EventEffect;
use crate::event_watch::EventWatch;
use crate::expansion::Expansion;
use crate::hover_panel::HoverPanel;
use crate::main_screen::MainScreen;
use crate::navigation::Navigation;
use crate::recommendations_screen::RecommendationsScreen;
use crate::scan_job::ScanJob;
use crate::scanning_screen::{ScanningScreen, center_lines};
use crate::screen_views::ScreenViews;
use crate::section::Section;
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
    APP_NAME, CLONED, DIAGRAM, FAILURE_CLOSE, FAILURE_TITLE, FDA_SETTINGS_URL, NOT_CLONED, RESTORE_FAILED,
    RESTORE_NOT_IN_TRASH, RESTORE_PARENT_MISSING, RESTORE_REPLACED,
};
use crate::tilde::tilde;
use crate::tray_model::TrayModel;
use crate::tray_screen::TrayScreen;
use crate::tray_sheet::TraySheet;

const SCAN_TICK: f64 = 1.0 / 30.0;
const SEARCH_TICK: f64 = 1.0 / 10.0;
const EVENTS_TICK: f64 = 0.5;
const OPEN: &str = "/usr/bin/open";

#[derive(Debug)]
pub(crate) struct ShellController {
    marker: MainThreadMarker,
    this: Weak<ShellController>,
    window: Retained<NSWindow>,
    root: Retained<ShellRootView>,
    actions: Retained<ShellActions>,
    hover: HoverPanel,
    paths: SessionPaths,
    state: RefCell<ShellState>,
}

impl ShellController {
    pub(crate) fn launch(marker: MainThreadMarker, paths: SessionPaths) -> Rc<Self> {
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
            paths,
            state: RefCell::new(ShellState::new(remembered_mode())),
        });
        let handler: Weak<dyn ActionHandler> = controller.this.clone();
        controller.actions.connect(handler);
        let appearance = controller.this.clone();
        controller.root.on_appearance_change(Rc::new(move || {
            if let Some(controller) = appearance.upgrade() {
                controller.restyle();
            }
        }));
        controller.observe_contrast();
        controller.window.center();
        controller.window.makeKeyAndOrderFront(None);
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

    pub(crate) fn application_will_terminate(&self) {
        let state = self.state.borrow();
        if let Some(job) = &state.scan {
            job.stop();
        }
        if let Some(job) = &state.search {
            job.stop();
        }
        if let Some(session) = &state.session
            && let Err(error) = session.persist()
        {
            eprintln!("domovoi: {error}");
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

    fn begin_scan(&self, session: Session, full_first_scan: bool) {
        let access = AccessState::probe();
        let skip_access = self.state.borrow().scan_without_access;
        if access == AccessState::Missing && !skip_access {
            self.state.borrow_mut().session = Some(session);
            return self.show_access(access);
        }
        let timer = self.repeating(SCAN_TICK, ShellController::scan_tick);
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
            }
            if job.is_finished() { scan.take() } else { None }
        };
        if let Some(job) = finished {
            let status = job.status.clone();
            let (session, outcome) = job.finish();
            self.state.borrow_mut().last_scan = Some(status);
            match outcome {
                Ok(()) | Err(SessionError::Scan(ScanError::Cancelled)) => self.install(session),
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
        let effect = {
            let mut state = self.state.borrow_mut();
            let ShellState { watch, session, .. } = &mut *state;
            let (Some(watch), Some(session)) = (watch.as_ref(), session.as_mut()) else {
                return;
            };
            let events: Vec<_> = watch.events.try_iter().collect();
            if events.is_empty() {
                return;
            }
            session.apply_events(&events)
        };
        match effect {
            Ok(EventEffect::Ignored) => {}
            Ok(EventEffect::Applied) => self.refresh(),
            Ok(EventEffect::RescanRequired) | Err(SessionError::Scan(_)) => {
                let session = self.state.borrow_mut().session.take();
                if let Some(session) = session {
                    self.begin_scan(session, false);
                }
            }
            Err(error) => self.fail(&error.to_string()),
        }
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
                let tokens = state.palette.tokens;
                let focus = state.focus;
                diagram.update_state(|diagram| {
                    let reduce_motion = diagram.reduce_motion;
                    *diagram = DiagramState::new(tree, diagram.mode, DIAGRAM, tokens, diagram.width, diagram.height);
                    diagram.reduce_motion = reduce_motion;
                    diagram.jump_to(focus);
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
        let marker = self.marker;
        let mut state = self.state.borrow_mut();
        state.palette = self.palette();
        let palette = state.palette;
        let previous = state.views.take();
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
                );
                let screen = ScanningScreen::build(marker, &data, &palette, &self.actions);
                let diagram = match previous {
                    Some(ScreenViews::Scanning { diagram, .. }) => diagram,
                    _ => match self.diagram_view(ScanningScreen::diagram_state(&data, &palette)) {
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
            (Stage::Main, Section::Disk | Section::Assistant | Section::Model) => {
                self.size_window(WINDOW, true);
                let data = state.data.as_ref().expect("the main stage has data");
                let screen = MainScreen::build(marker, data, state.focus, state.mode, &palette, &self.actions);
                let diagram = match previous {
                    Some(ScreenViews::Main { diagram, .. }) => diagram,
                    _ => match self.diagram_view(MainScreen::diagram_state(data, state.focus, state.mode, &palette)) {
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
        let content: Retained<NSView> = match &views {
            ScreenViews::Access(screen) => screen.root.clone().into_super(),
            ScreenViews::Scanning { screen, .. } => screen.frame.root(),
            ScreenViews::Main { screen, .. } => screen.frame.root(),
            ScreenViews::Recommendations(screen) => screen.frame.root(),
            ScreenViews::Duplicates(screen) => screen.frame.root(),
        };
        content.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        self.root.show(&content);
        state.views = Some(views);
    }

    fn diagram_view(&self, mut diagram: DiagramState) -> Result<Retained<DiagramView>, String> {
        diagram.reduce_motion = reduce_motion();
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
            {
                let mut state = self.state.borrow_mut();
                state.section = section;
                state.views = None;
            }
            self.show_current();
        }
    }

    fn toggle_card(&self, position: usize) {
        let mut state = self.state.borrow_mut();
        let model = &mut state.care_mut().recommendations;
        if let Some(card) = model.cards.get_mut(position).filter(|card| card.selectable()) {
            card.selected = !card.selected;
        }
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
                .filter(|card| card.selected && card.selectable())
                .flat_map(|card| card.targets.clone())
                .collect();
            let session = state
                .session
                .as_mut()
                .expect("recommendations come from an open session");
            for target in targets {
                session.add_to_tray(target.node, target.rule_id.as_deref());
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
                for (_, copy) in &pairs {
                    session.add_to_tray(*copy, None);
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
        let tray = {
            let state = self.state.borrow();
            let session = state.session.as_ref().expect("the tray belongs to an open session");
            let index = session.index().expect("the tray belongs to a scanned volume");
            TrayModel::build(index, session.tray(), session.trash_journal())
        };
        match tray {
            Ok(tray) => self.show_tray(&tray, false),
            Err(error) => self.fail(&error.to_string()),
        }
    }

    fn show_tray(&self, model: &TrayModel, stitch: bool) {
        let palette = self.state.borrow().palette;
        let screen = TrayScreen::build(self.marker, model, &palette, &self.actions);
        let existing = self.state.borrow_mut().tray.take();
        let window = if let Some(sheet) = existing {
            sheet.window
        } else {
            let window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(self.marker),
                    NSRect::new(NSPoint::ZERO, screen.size()),
                    NSWindowStyleMask::Titled,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };
            unsafe { window.setReleasedWhenClosed(false) };
            self.window.beginSheet_completionHandler(&window, None);
            window
        };
        window.setContentSize(screen.size());
        window.setContentView(Some(&screen.root));
        if stitch && let Some(ornament) = &screen.ornament {
            ornament.stitch_once(reduce_motion());
        }
        self.state.borrow_mut().tray = Some(TraySheet { window, screen });
    }

    fn refresh_tray(&self) {
        let confirming = self
            .state
            .borrow()
            .tray
            .as_ref()
            .is_some_and(|sheet| sheet.screen.ornament.is_none());
        if confirming {
            self.open_tray();
        }
    }

    fn confirm_tray(&self) {
        let outcome = {
            let mut state = self.state.borrow_mut();
            let session = state.session.as_mut().expect("the tray belongs to an open session");
            session
                .tray_reclaimable()
                .map_err(SessionError::Io)
                .and_then(|reclaimable| Ok((reclaimable.bytes, session.move_tray_to_trash()?)))
        };
        let (moved, outcomes) = match outcome {
            Ok(outcome) => outcome,
            Err(error) => return self.fail(&error.to_string()),
        };
        let rebuilt = self.state.borrow_mut().rebuild_data(SystemTime::now());
        if let Err(error) = rebuilt {
            return self.fail(&error.to_string());
        }
        let model = {
            let state = self.state.borrow();
            let session = state.session.as_ref().expect("the tray belongs to an open session");
            TrayModel::default().with_outcome(&outcomes, moved, session.trash_journal())
        };
        self.show_tray(&model, true);
        self.show_current();
    }

    fn close_tray(&self) {
        let sheet = self.state.borrow_mut().tray.take();
        if let Some(sheet) = sheet {
            self.window.endSheet(&sheet.window);
            sheet.window.orderOut(None);
        }
        self.show_current();
    }

    fn restore(&self, position: usize) {
        let restored = {
            let mut state = self.state.borrow_mut();
            state
                .session
                .as_mut()
                .expect("the journal belongs to an open session")
                .restore(position)
        };
        if let Err(error) = restored {
            let reason = match &error {
                RestoreError::NotInTrash => RESTORE_NOT_IN_TRASH.to_owned(),
                RestoreError::Replaced => RESTORE_REPLACED.to_owned(),
                RestoreError::ParentMissing => RESTORE_PARENT_MISSING.to_owned(),
                RestoreError::Io(error) => error.to_string(),
            };
            self.inform(&format!("{RESTORE_FAILED}: {reason}"));
        }
        self.refresh();
        let showing_outcome = self
            .state
            .borrow()
            .tray
            .as_ref()
            .is_some_and(|sheet| sheet.screen.ornament.is_some());
        if showing_outcome {
            self.open_tray();
        }
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
        let controller = self.this.clone();
        let block = RcBlock::new(move |_: NonNull<NSTimer>| {
            if let Some(controller) = controller.upgrade() {
                tick(&controller);
            }
        });
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, true, &block) }
    }
}

impl ActionHandler for ShellController {
    fn perform(&self, action: ShellAction) {
        match action {
            ShellAction::OpenSettings => {
                self.state.borrow_mut().settings_opened = true;
                let url =
                    NSURL::URLWithString(&NSString::from_str(FDA_SETTINGS_URL)).expect("the settings URL is valid");
                NSWorkspace::sharedWorkspace().openURL(&url);
            }
            ShellAction::ScanWithoutAccess => {
                self.state.borrow_mut().scan_without_access = true;
                let session = self.state.borrow_mut().session.take();
                match session {
                    Some(session) => self.begin_scan(session, false),
                    None => self.open_session(),
                }
            }
            ShellAction::Relaunch => self.relaunch(),
            ShellAction::StopScan => {
                if let Some(job) = &self.state.borrow().scan {
                    job.stop();
                }
            }
            ShellAction::ChangeMode(mode) => {
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
            ShellAction::Focus(node) => self.focus_on(node),
            ShellAction::ShowInFinder(node) => self.show_in_finder(node),
            ShellAction::ShowClickedRowInFinder => {
                if let Some(node) = self.clicked_row() {
                    self.show_in_finder(node);
                }
            }
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
            ShellAction::SendCardsToTray => self.send_cards_to_tray(),
            ShellAction::ToggleDuplicateSearch => self.toggle_duplicate_search(),
            ShellAction::SelectDuplicateAction(clone) => self.select_duplicate_action(clone),
            ShellAction::ToggleCopy(tag) => self.toggle_copy(tag),
            ShellAction::ApplyDuplicates => self.apply_duplicates(),
            ShellAction::ConfirmTray => self.confirm_tray(),
            ShellAction::CloseTray => self.close_tray(),
            ShellAction::OpenTrash => {
                if let Err(error) = open_trash() {
                    self.inform(&error.to_string());
                }
            }
            ShellAction::Restore(position) => self.restore(position),
        }
    }

    fn allows(&self, action: ShellAction) -> bool {
        match action {
            ShellAction::ShowInFinder(node) => self.state.borrow().node_path(node).is_some(),
            ShellAction::ShowClickedRowInFinder => self
                .clicked_row()
                .is_some_and(|node| self.state.borrow().node_path(node).is_some()),
            _ => true,
        }
    }
}

fn place_diagram(diagram: &DiagramView, pane: &NSView, frame: NSRect) {
    diagram.removeFromSuperview();
    diagram.setFrame(frame);
    diagram.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinYMargin);
    pane.addSubview(diagram);
}
