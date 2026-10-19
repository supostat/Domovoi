use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};

use domovoi_app::{
    CardsSource, RecommendationsScreen, Screen, ShellActions, ShellData, ShellFixture, ShellPalette, WINDOW,
    development_fonts,
};
use domovoi_render::register_fonts;
use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::rc::{Retained, autoreleasepool};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSCollectionView, NSCollectionViewFlowLayout, NSScrollView, NSScrollerStyle,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSRunLoop, NSSize};

const COPIES: usize = 8;
const LAYOUT_PASSES: usize = 10;
const WARM_UP: usize = 20;
const REBUILDS: usize = 200;
const NARROWER_BY: f64 = 120.0;
const LEAK_ALLOWANCE: u64 = 2;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn long_list() -> ShellData {
    let mut data = ShellFixture::read(&fixture())
        .and_then(|fixture| fixture.shell_data(Screen::Recommendations))
        .expect("the fixture builds the recommendations screen data");
    let cards = data.care.recommendations.cards.clone();
    data.care.recommendations.cards = (0..COPIES).flat_map(|_| cards.iter().cloned()).collect();
    data
}

fn palette() -> ShellPalette {
    ShellPalette {
        tokens: &Palette::DARK,
        high_contrast: false,
    }
}

fn phys_footprint() -> u64 {
    let mut usage: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
    let pid = libc::c_int::try_from(process::id()).expect("a pid fits in c_int");
    let status = unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V2, (&raw mut usage).cast()) };
    assert_eq!(status, 0, "proc_pid_rusage reads the own process");
    usage.ri_phys_footprint
}

fn turn_run_loop() {
    let now = NSDate::dateWithTimeIntervalSinceNow(0.0);
    NSRunLoop::currentRunLoop().runMode_beforeDate(unsafe { NSDefaultRunLoopMode }, &now);
}

fn offscreen_window(marker: MainThreadMarker) -> Retained<NSWindow> {
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(marker),
            NSRect::new(NSPoint::ZERO, WINDOW),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window
}

struct Probe<'window> {
    window: &'window NSWindow,
    screen: RecommendationsScreen,
}

impl<'window> Probe<'window> {
    fn show(window: &'window NSWindow, marker: MainThreadMarker, data: &ShellData, actions: &ShellActions) -> Self {
        let screen = RecommendationsScreen::build(marker, data, &palette(), actions);
        window.setContentView(Some(&screen.frame.root()));
        let probe = Self { window, screen };
        probe.scroll().setScrollerStyle(NSScrollerStyle::Legacy);
        probe
    }

    fn collection(&self) -> &NSCollectionView {
        &self.screen.collection.as_ref().expect("the fixture has cards").0
    }

    fn source(&self) -> &CardsSource {
        &self.screen.collection.as_ref().expect("the fixture has cards").1
    }

    fn scroll(&self) -> Retained<NSScrollView> {
        self.collection().enclosingScrollView().expect("the cards scroll")
    }

    fn lay_out(&self) {
        self.scroll().tile();
        self.window
            .contentView()
            .expect("the window shows the screen")
            .layoutSubtreeIfNeeded();
        self.collection().layoutSubtreeIfNeeded();
    }

    fn card_widths(&self) -> Vec<f64> {
        let count = usize::try_from(self.collection().numberOfItemsInSection(0)).expect("a count is non-negative");
        (0..count)
            .map(|position| self.collection().frameForItemAtIndex(position).size.width)
            .collect()
    }

    fn available_width(&self) -> f64 {
        let layout = self
            .collection()
            .collectionViewLayout()
            .and_then(|layout| layout.downcast::<NSCollectionViewFlowLayout>().ok())
            .expect("the cards use a flow layout");
        let insets = layout.sectionInset();
        self.collection().bounds().size.width - insets.left - insets.right
    }

    fn resize(&self, width: f64) {
        self.window.setContentSize(NSSize::new(width, WINDOW.height));
    }
}

#[derive(Default)]
struct Checks {
    failures: Vec<String>,
}

impl Checks {
    fn expect(&mut self, condition: bool, description: String) {
        println!("{} {description}", if condition { "ok  " } else { "FAIL" });
        if !condition {
            self.failures.push(description);
        }
    }
}

fn layout_contract(marker: MainThreadMarker, data: &ShellData, actions: &ShellActions, checks: &mut Checks) {
    let window = offscreen_window(marker);
    let probe = Probe::show(&window, marker, data, actions);
    let mut passes = Vec::with_capacity(LAYOUT_PASSES);
    for _ in 0..LAYOUT_PASSES {
        probe.lay_out();
        passes.push(probe.card_widths());
    }
    let available = probe.available_width();
    let widest = passes.iter().flatten().copied().fold(0.0_f64, f64::max);
    checks.expect(
        widest < available,
        format!("1) the widest card {widest} pt is narrower than the available {available} pt"),
    );
    let distinct: Vec<f64> = passes.iter().flatten().fold(Vec::new(), |mut seen, width| {
        if !seen.iter().any(|known: &f64| (known - width).abs() < f64::EPSILON) {
            seen.push(*width);
        }
        seen
    });
    checks.expect(
        distinct.len() == 1,
        format!("2) card widths over {LAYOUT_PASSES} layout passes: {distinct:?}"),
    );
    let settled = probe.source().measurements();
    checks.expect(
        settled == 1,
        format!("3) height measurements after building and {LAYOUT_PASSES} passes: {settled}"),
    );
    probe.resize(WINDOW.width - NARROWER_BY);
    probe.lay_out();
    let resized = probe.source().measurements();
    checks.expect(
        resized == settled + 1,
        format!("3) height measurements after a narrower window: {resized} (before: {settled})"),
    );
    let narrowed = probe.available_width();
    let widest_narrowed = probe.card_widths().into_iter().fold(0.0_f64, f64::max);
    checks.expect(
        widest_narrowed < narrowed,
        format!("1) after a narrower window the widest card {widest_narrowed} pt is narrower than the available {narrowed} pt"),
    );
}

fn memory_per_rebuild(marker: MainThreadMarker, data: &ShellData, actions: &ShellActions, checks: &mut Checks) {
    let window = offscreen_window(marker);
    let rebuild = || {
        autoreleasepool(|_| {
            Probe::show(&window, marker, data, actions).lay_out();
        });
        turn_run_loop();
    };
    let before = phys_footprint();
    rebuild();
    let first = phys_footprint().saturating_sub(before);
    for _ in 0..WARM_UP {
        rebuild();
    }
    let warmed = phys_footprint();
    for _ in 0..REBUILDS {
        rebuild();
    }
    let growth = phys_footprint().saturating_sub(warmed);
    checks.expect(
        growth <= LEAK_ALLOWANCE * first,
        format!(
            "4) footprint growth over {REBUILDS} rebuilds after warm-up: {growth} bytes; the first rebuild: {first} bytes"
        ),
    );
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the layout check runs on the main thread");
    let _application = NSApplication::sharedApplication(marker);
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let data = long_list();
    let actions = ShellActions::new(marker);
    let mut checks = Checks::default();
    layout_contract(marker, &data, &actions, &mut checks);
    memory_per_rebuild(marker, &data, &actions, &mut checks);
    if checks.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!("the cards layout breaks its contract:\n{}", checks.failures.join("\n"));
        ExitCode::FAILURE
    }
}
