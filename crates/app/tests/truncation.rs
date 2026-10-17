use std::path::Path;
use std::process::ExitCode;

use domovoi_app::{
    Breadcrumbs, Crumb, HoverPanel, HoverTip, LevelRow, LevelRowView, ScanningScreen, ShellActions, ShellFixture,
    ShellPalette, SidebarEntry, SidebarItemView, development_fonts, strings,
};
use domovoi_index::Category;
use domovoi_render::register_fonts;
use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAccessibility, NSControl, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

const ROW_HEIGHT: f64 = 30.0;
const NARROW_ROW: f64 = 180.0;
const SIDEBAR_WIDTH: f64 = 230.0;
const SIDEBAR_ROW: f64 = 28.0;
const LONG_NAME: &str = "Очень длинное имя папки проекта, которое не помещается в строку списка уровня";
const LONG_PATH: &str = "/Users/igor/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw/very/deep/nested/folder/structure/with/a/final/file.bin";
const SIZE_TEXT: &str = "393,2 ГБ";
const CRUMBS: [&str; 7] = [
    "Macintosh HD",
    "Пользователи · igor",
    "Library",
    "Containers",
    "com.docker.docker",
    "Data",
    "vms",
];

#[derive(Default)]
struct Checks {
    failures: Vec<String>,
    count: usize,
}

impl Checks {
    fn expect(&mut self, condition: bool, description: impl Into<String>) {
        self.count += 1;
        if !condition {
            self.failures.push(description.into());
        }
    }

    fn mode(&mut self, field: &NSControl, expected: NSLineBreakMode, element: &str) {
        let actual = field.lineBreakMode();
        self.expect(
            actual == expected,
            format!("{element}: line break {actual:?}, expected {expected:?}"),
        );
    }

    fn never_cut(&mut self, field: &NSTextField, element: &str) {
        let shown = field.frame().size.width;
        let needed = field.fittingSize().width;
        self.expect(
            shown + 0.5 >= needed,
            format!("{element}: {shown} pt shown of {needed} pt"),
        );
    }

    fn truncated_with_full_text(&mut self, field: &NSView, text: &str, element: &str) {
        let tooltip = field.toolTip().map(|tooltip| tooltip.to_string());
        self.expect(
            tooltip.as_deref() == Some(text),
            format!("{element}: tooltip {tooltip:?}"),
        );
        let label = field.accessibilityLabel().map(|label| label.to_string());
        self.expect(
            label.as_deref() == Some(text),
            format!("{element}: VoiceOver label {label:?}"),
        );
    }
}

fn palette() -> ShellPalette {
    ShellPalette {
        tokens: &Palette::DARK,
        high_contrast: false,
    }
}

fn container(marker: MainThreadMarker, width: f64, height: f64) -> Retained<NSView> {
    NSView::initWithFrame(
        NSView::alloc(marker),
        NSRect::new(NSPoint::ZERO, NSSize::new(width, height)),
    )
}

fn level_rows(checks: &mut Checks, marker: MainThreadMarker) {
    let row = LevelRow {
        node: 1,
        name: LONG_NAME.to_owned(),
        size_text: SIZE_TEXT.to_owned(),
        share: 1.0,
        category: Category::Dev,
    };
    for width in [360.0, NARROW_ROW] {
        let host = container(marker, width, ROW_HEIGHT);
        let parts = LevelRowView::build(marker, &host, &row, &palette());
        let element = format!("level row at {width} pt");
        checks.mode(
            &parts.name,
            NSLineBreakMode::ByTruncatingTail,
            &format!("{element}, name"),
        );
        checks.expect(
            parts.name.frame().size.width < parts.name.fittingSize().width,
            format!("{element}: the long name is shortened"),
        );
        checks.truncated_with_full_text(&parts.name, LONG_NAME, &format!("{element}, name"));
        checks.mode(&parts.size, NSLineBreakMode::ByClipping, &format!("{element}, size"));
        checks.never_cut(&parts.size, &format!("{element}, size"));
    }
}

fn sidebar(checks: &mut Checks, marker: MainThreadMarker) {
    let entry = SidebarEntry {
        title: LONG_NAME.to_owned(),
        counter: Some(SIZE_TEXT.to_owned()),
        children: Vec::new(),
        selected: false,
    };
    let item = SidebarItemView::build(marker, &entry, SIDEBAR_WIDTH, SIDEBAR_ROW, &palette());
    checks.mode(&item.name, NSLineBreakMode::ByTruncatingTail, "sidebar name");
    checks.truncated_with_full_text(&item.name, LONG_NAME, "sidebar name");
    let counter = item.counter.as_ref().expect("the item shows its counter");
    checks.never_cut(counter, "sidebar counter");
    checks.expect(
        item.name.frame().origin.x + item.name.frame().size.width <= counter.frame().origin.x,
        "sidebar name stops before the counter",
    );
}

fn crumbs(checks: &mut Checks, marker: MainThreadMarker) {
    let path: Vec<Crumb> = CRUMBS
        .iter()
        .enumerate()
        .map(|(node, title)| Crumb {
            node,
            title: (*title).to_owned(),
        })
        .collect();
    let actions = ShellActions::new(marker);
    let crumbs = Breadcrumbs::build(marker, &path, &palette(), &actions);
    let titles: Vec<String> = crumbs.buttons.iter().map(|button| button.title().to_string()).collect();
    checks.expect(
        titles == ["Macintosh HD", "…", "Data", "vms"],
        format!("crumbs collapse the middle levels: {titles:?}"),
    );
    let collapsed: Vec<&str> = crumbs.collapsed.iter().map(|crumb| crumb.title.as_str()).collect();
    checks.expect(collapsed == CRUMBS[1..5], format!("collapsed levels {collapsed:?}"));
    let menu = crumbs.buttons[1].menu().expect("the ellipsis carries a menu");
    let entries: Vec<String> = menu.itemArray().iter().map(|item| item.title().to_string()).collect();
    checks.expect(entries == CRUMBS[1..5], format!("the ellipsis menu lists {entries:?}"));
    for (button, title) in crumbs
        .buttons
        .iter()
        .zip(titles.iter())
        .filter(|(_, title)| *title != "…")
    {
        checks.mode(button, NSLineBreakMode::ByTruncatingTail, &format!("crumb {title}"));
        checks.truncated_with_full_text(button, title, &format!("crumb {title}"));
        let show = button.menu().expect("a crumb carries a menu");
        let first = show
            .itemArray()
            .firstObject()
            .expect("the menu has an item")
            .title()
            .to_string();
        checks.expect(
            first == strings::SHOW_IN_FINDER,
            format!("crumb {title} menu offers {first}"),
        );
    }
}

fn scanning(checks: &mut Checks, marker: MainThreadMarker) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json");
    let mut data = ShellFixture::read(&fixture)
        .and_then(|fixture| fixture.shell_data())
        .expect("the fixture reads");
    LONG_PATH.clone_into(&mut data.scan.current);
    let actions = ShellActions::new(marker);
    let screen = ScanningScreen::build(marker, &data, &palette(), &actions);
    checks.mode(&screen.path, NSLineBreakMode::ByTruncatingMiddle, "scanned path");
    checks.truncated_with_full_text(&screen.path, LONG_PATH, "scanned path");
    checks.never_cut(&screen.engine, "scan engine line");
}

fn hover(checks: &mut Checks, marker: MainThreadMarker) {
    let panel = HoverPanel::new(marker);
    panel.fill(
        &HoverTip {
            name: LONG_NAME.to_owned(),
            size_text: SIZE_TEXT.to_owned(),
            path: Some(LONG_PATH.to_owned()),
            source: strings::SOURCE_SCAN,
        },
        &palette(),
    );
    checks.mode(&panel.path, NSLineBreakMode::ByTruncatingMiddle, "hover path");
    checks.truncated_with_full_text(&panel.path, LONG_PATH, "hover path");
    checks.mode(&panel.name, NSLineBreakMode::ByTruncatingTail, "hover name");
    checks.truncated_with_full_text(&panel.name, LONG_NAME, "hover name");
    checks.never_cut(&panel.size, "hover size");
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the truncation check runs on the main thread");
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let mut checks = Checks::default();
    level_rows(&mut checks, marker);
    sidebar(&mut checks, marker);
    crumbs(&mut checks, marker);
    scanning(&mut checks, marker);
    hover(&mut checks, marker);
    if checks.failures.is_empty() {
        println!("truncation: {} checks passed", checks.count);
        ExitCode::SUCCESS
    } else {
        eprintln!("truncation failed:\n{}", checks.failures.join("\n"));
        ExitCode::FAILURE
    }
}
