use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::time::Instant;

use block2::RcBlock;
use domovoi_render::{
    DiagramChange, DiagramGraphics, DiagramObserver, DiagramRenderer, DiagramState, DiagramView, GlyphAtlas,
    NodeOrigin, ROOT,
};
use domovoi_scanner::ScanError;
use domovoi_theme::Palette;
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
use crate::diagram_preference::{remember_mode, remembered_mode};
use crate::disk_overview::{DiskOverview, SYSTEM_DATA_ROOT};
use crate::event_effect::EventEffect;
use crate::event_watch::EventWatch;
use crate::expansion::{Expansion, NODE_BUDGET};
use crate::hover_panel::HoverPanel;
use crate::main_screen::MainScreen;
use crate::running_applications::running_bundle_ids;
use crate::scan_job::ScanJob;
use crate::scanning_screen::{ScanningScreen, center_lines};
use crate::screen_views::ScreenViews;
use crate::session::Session;
use crate::session_error::SessionError;
use crate::shell_action::ShellAction;
use crate::shell_actions::ShellActions;
use crate::shell_data::ShellData;
use crate::shell_layout::WINDOW;
use crate::shell_palette::ShellPalette;
use crate::shell_root_view::ShellRootView;
use crate::shell_state::ShellState;
use crate::stage::Stage;
use crate::strings::{APP_NAME, DIAGRAM, FAILURE_CLOSE, FAILURE_TITLE, FDA_SETTINGS_URL};

const CACHE_FILE: &str = "scan.cache";
const SCAN_TICK: f64 = 1.0 / 30.0;
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
    cache: PathBuf,
    state: RefCell<ShellState>,
}

impl ShellController {
    pub(crate) fn launch(marker: MainThreadMarker, cache_directory: &Path) -> Rc<Self> {
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
            cache: cache_directory.join(CACHE_FILE),
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
            let session = Session::open(Path::new(SYSTEM_DATA_ROOT), &self.cache, env!("CARGO_PKG_VERSION"))?;
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

    fn install(&self, mut session: Session) {
        if let Err(error) = session.refresh_recommendations(&running_bundle_ids()) {
            return self.fail(&error.to_string());
        }
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
            state.expansion = session.index().map_or_else(Expansion::default, |index| {
                Expansion::around(index, index.root(), NODE_BUDGET)
            });
            state.session = Some(session);
            state.watch = watch;
            state.stage = Stage::Main;
            state.focus = ROOT;
            state.views = None;
        }
        self.show_current();
    }

    fn events_tick(&self) {
        let mut applied = false;
        let mut rescan = false;
        {
            let mut state = self.state.borrow_mut();
            let ShellState { watch, session, .. } = &mut *state;
            let (Some(watch), Some(session)) = (watch.as_ref(), session.as_mut()) else {
                return;
            };
            while let Ok(event) = watch.events.try_recv() {
                match session.apply_event(&event) {
                    Ok(EventEffect::Applied) => applied = true,
                    Ok(EventEffect::Ignored) => {}
                    Ok(EventEffect::RescanRequired) | Err(_) => rescan = true,
                }
            }
        }
        if rescan {
            let session = self.state.borrow_mut().session.take();
            if let Some(session) = session {
                self.begin_scan(session, false);
            }
        } else if applied {
            let refreshed = {
                let mut state = self.state.borrow_mut();
                let session = state.session.as_mut().expect("events arrive for an open session");
                session.refresh_recommendations(&running_bundle_ids())
            };
            match refreshed {
                Ok(()) => self.refresh_main(),
                Err(error) => self.fail(&error.to_string()),
            }
        }
    }

    fn refresh_main(&self) {
        let mut state = self.state.borrow_mut();
        let focus_origin = state.data.as_ref().map(|data| data.tree.node(state.focus).origin);
        let data = state.shell_data();
        let focus = focus_origin
            .and_then(|origin| data.tree_position(origin))
            .unwrap_or(ROOT);
        state.focus = focus;
        state.data = Some(data);
        if let Some(diagram) = state.views.as_ref().and_then(ScreenViews::diagram) {
            let tree = state.data.as_ref().expect("the data was just built").tree.clone();
            let palette = state.palette;
            diagram.update_state(|diagram| {
                let reduce_motion = diagram.reduce_motion;
                *diagram = DiagramState::new(
                    tree,
                    diagram.mode,
                    DIAGRAM,
                    palette.tokens,
                    diagram.width,
                    diagram.height,
                );
                diagram.reduce_motion = reduce_motion;
                diagram.jump_to(focus);
            });
        }
        drop(state);
        self.show_current();
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
        let views = match state.stage {
            Stage::Access(access) => {
                let screen = AccessScreen::build(marker, access, &palette, &self.actions);
                self.size_window(screen.size(), false);
                ScreenViews::Access(screen)
            }
            Stage::Scanning => {
                self.size_window(WINDOW, true);
                let status = state.scan.as_ref().expect("a scan runs while scanning").status.clone();
                let data = ShellData::measured(state.access, state.overview(), None, &Expansion::default(), status);
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
            Stage::Main => {
                self.size_window(WINDOW, true);
                if state.data.is_none() {
                    let data = state.shell_data();
                    state.data = Some(data);
                }
                let data = state.data.as_ref().expect("the main screen has data");
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
        };
        content.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        self.root.show(&content);
        state.views = Some(views);
    }

    fn diagram_view(&self, mut diagram: DiagramState) -> Result<Retained<DiagramView>, String> {
        diagram.reduce_motion = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
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
            state.stage == Stage::Main && state.focus != change.focus
        };
        if focus_changed {
            self.focus_on(change.focus);
        }
    }

    fn focus_on(&self, node: usize) {
        let mut state = self.state.borrow_mut();
        state.focus = node;
        let origin = state.data.as_ref().map(|data| data.tree.node(node).origin);
        if let Some(NodeOrigin::Indexed(id)) = origin {
            let needs_expansion = state
                .session
                .as_ref()
                .and_then(Session::index)
                .is_some_and(|index| !state.expansion.shows_rings_below(index, id));
            if needs_expansion {
                let index = state
                    .session
                    .as_ref()
                    .and_then(Session::index)
                    .expect("an indexed node has an index");
                state.expansion = Expansion::around(index, id, NODE_BUDGET);
                drop(state);
                return self.refresh_main();
            }
        }
        if let Some(diagram) = state.views.as_ref().and_then(ScreenViews::diagram) {
            diagram.update_state(|diagram| diagram.set_focus(node, Instant::now()));
        }
        drop(state);
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
                let mut state = self.state.borrow_mut();
                state.mode = mode;
                if let Some(diagram) = state.views.as_ref().and_then(ScreenViews::diagram) {
                    diagram.update_state(|diagram| diagram.set_mode(mode));
                }
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
