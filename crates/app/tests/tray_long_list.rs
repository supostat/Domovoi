use std::process::ExitCode;

use domovoi_app::{
    DeleteOutcome, DeleteStatus, ShellActions, ShellPalette, TrayModel, TrayRow, TrayScreen, WINDOW, development_fonts,
};
use domovoi_index::Category;
use domovoi_render::register_fonts;
use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSScrollView, NSView};
use objc2_foundation::NSPoint;

const MANY: usize = 40;
const FEW: usize = 2;
const SCROLLED: f64 = 300.0;
const TOLERANCE: f64 = 0.5;

#[derive(Default)]
struct Checks {
    failures: Vec<String>,
}

impl Checks {
    fn expect(&mut self, condition: bool, description: impl Into<String>) {
        let description = description.into();
        println!("{} {description}", if condition { "ok  " } else { "FAIL" });
        if !condition {
            self.failures.push(description);
        }
    }
}

fn palette() -> ShellPalette {
    ShellPalette {
        tokens: &Palette::DARK,
        high_contrast: false,
    }
}

fn tray(rows: usize) -> TrayModel {
    TrayModel {
        rows: (0..rows)
            .map(|number| TrayRow {
                category: Category::Dev,
                title: format!("project-{number:02}"),
                size: 40_000_000,
                details: vec![
                    format!("~/Projects/project-{number:02}/node_modules"),
                    format!("~/Projects/project-{number:02}/.next"),
                ],
                unmarked: Vec::new(),
            })
            .collect(),
        ..TrayModel::default()
    }
}

fn offset(scroll: &NSScrollView) -> f64 {
    scroll.contentView().bounds().origin.y
}

fn scroll_to(scroll: &NSScrollView, top: f64) {
    let clip = scroll.contentView();
    clip.scrollToPoint(NSPoint::new(0.0, top));
    scroll.reflectScrolledClipView(&clip);
}

fn fits_the_window(checks: &mut Checks, screen: &TrayScreen, state: &str) -> Retained<NSScrollView> {
    let height = screen.size().height;
    checks.expect(
        height <= WINDOW.height,
        format!(
            "{state}: the sheet of {height} pt fits a window of {} pt",
            WINDOW.height
        ),
    );
    let button: &NSView = &screen.primary;
    let frame = button.frame();
    checks.expect(
        frame.origin.y >= 0.0 && frame.origin.y + frame.size.height <= height,
        format!("{state}: the primary button {frame:?} is inside the sheet"),
    );
    let probe = screen
        .probes
        .accent_button
        .expect("the sheet probes its primary button");
    checks.expect(
        (probe[1] - (height - frame.origin.y - frame.size.height)).abs() < TOLERANCE,
        format!("{state}: the accent probe {probe:?} stands on the primary button"),
    );
    let scroll = screen.body.clone().expect("a long sheet scrolls its body");
    let document = scroll
        .documentView()
        .expect("the body has a document")
        .frame()
        .size
        .height;
    checks.expect(
        document > scroll.frame().size.height,
        format!(
            "{state}: the body of {document} pt scrolls in {} pt",
            scroll.frame().size.height
        ),
    );
    scroll
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the tray check runs on the main thread");
    let _application = NSApplication::sharedApplication(marker);
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let actions = ShellActions::new(marker);
    let mut checks = Checks::default();

    let long = tray(MANY);
    let confirming = TrayScreen::build(marker, &long, &palette(), &actions, WINDOW.height);
    let scroll = fits_the_window(&mut checks, &confirming, "confirming");
    checks.expect(confirming.rows.len() == MANY, "every row is in the sheet");

    scroll_to(&scroll, SCROLLED);
    let rebuilt = TrayScreen::build(marker, &long, &palette(), &actions, WINDOW.height);
    rebuilt.keep_place_of(&confirming);
    let kept = offset(rebuilt.body.as_ref().expect("the rebuilt sheet scrolls"));
    checks.expect(
        (kept - SCROLLED).abs() < TOLERANCE,
        format!("a rebuild keeps the list at {SCROLLED} pt (got {kept})"),
    );

    let deleting = long.clone().with_deleting(DeleteStatus {
        files_done: 1,
        files_total: 2,
    });
    let deleting = TrayScreen::build(marker, &deleting, &palette(), &actions, WINDOW.height);
    fits_the_window(&mut checks, &deleting, "deleting");

    let deleted = TrayModel::default().with_deleted(DeleteOutcome {
        objects: 1,
        freed: 1_000_000,
        failures: (0..MANY)
            .map(|number| format!("~/Projects/project-{number:02}: операция не позволена"))
            .collect(),
    });
    let deleted = TrayScreen::build(marker, &deleted, &palette(), &actions, WINDOW.height);
    fits_the_window(&mut checks, &deleted, "deleted");

    let short = TrayScreen::build(marker, &tray(FEW), &palette(), &actions, WINDOW.height);
    checks.expect(short.body.is_none(), "a short list stays without a scroll");

    if checks.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!("the tray sheet outgrows its window:\n{}", checks.failures.join("\n"));
        ExitCode::FAILURE
    }
}
