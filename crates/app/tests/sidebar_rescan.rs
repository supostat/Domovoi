use std::path::Path;
use std::process::ExitCode;

use domovoi_app::{
    Navigation, SIDEBAR_WIDTH, Screen, Section, ShellActions, ShellData, ShellFixture, ShellPalette, SidebarPanel,
    WINDOW, development_fonts, main_menu, strings,
};
use domovoi_render::register_fonts;
use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSApplication, NSView};
use objc2_foundation::{NSRect, NSSize, NSString};

const RESCAN_KEY: &str = "r";

#[derive(Default)]
struct Checks {
    failures: Vec<String>,
}

impl Checks {
    fn expect(&mut self, condition: bool, description: impl Into<String>) {
        if !condition {
            self.failures.push(description.into());
        }
    }
}

fn fixture_data() -> ShellData {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json");
    ShellFixture::read(&path)
        .and_then(|fixture| fixture.shell_data(Screen::Main))
        .expect("the shell fixture reads")
}

fn palette() -> ShellPalette {
    ShellPalette {
        tokens: &Palette::LIGHT,
        high_contrast: false,
    }
}

fn panel(marker: MainThreadMarker, data: &ShellData, actions: &ShellActions, rescan_allowed: bool) -> SidebarPanel {
    let data = ShellData {
        rescan_allowed,
        ..data.clone()
    };
    SidebarPanel::build(
        marker,
        NSSize::new(SIDEBAR_WIDTH, WINDOW.height),
        &data,
        Navigation {
            current: Section::Disk,
            care_ready: data.care_ready,
        },
        palette(),
        actions,
    )
}

fn overlaps(first: NSRect, second: NSRect) -> bool {
    first.origin.x < second.origin.x + second.size.width
        && second.origin.x < first.origin.x + first.size.width
        && first.origin.y < second.origin.y + second.size.height
        && second.origin.y < first.origin.y + first.size.height
}

fn button_contract(marker: MainThreadMarker, data: &ShellData, actions: &ShellActions, checks: &mut Checks) {
    let panel = panel(marker, data, actions, true);
    let title = panel.rescan.title().to_string();
    checks.expect(title == strings::RESCAN, format!("the button reads {title:?}"));
    checks.expect(
        panel.rescan.action() == Some(sel!(rescanDisk:)),
        "the button sends rescanDisk:",
    );
    checks.expect(
        panel
            .rescan
            .target()
            .is_some_and(|target| std::ptr::eq(Retained::as_ptr(&target), actions.target())),
        "the button targets the shell actions",
    );
}

fn follows_the_gate(marker: MainThreadMarker, data: &ShellData, actions: &ShellActions, checks: &mut Checks) {
    for allowed in [true, false] {
        let panel = panel(marker, data, actions, allowed);
        checks.expect(
            panel.rescan.isEnabled() == allowed,
            format!("the button is enabled = {allowed} when the gate says so"),
        );
    }
}

fn sits_in_the_disk_block(marker: MainThreadMarker, data: &ShellData, actions: &ShellActions, checks: &mut Checks) {
    let panel = panel(marker, data, actions, true);
    let frame = panel.rescan.frame();
    let inset = panel.free.frame().origin.x;
    checks.expect(
        frame.origin.x >= inset && frame.origin.x + frame.size.width <= SIDEBAR_WIDTH - inset,
        format!("the button keeps the sidebar insets: {frame:?}"),
    );
    let list = panel
        .outline
        .enclosingScrollView()
        .expect("the outline scrolls")
        .frame();
    checks.expect(
        frame.origin.y + frame.size.height <= list.origin.y,
        format!("the button sits below the list: {frame:?} vs {list:?}"),
    );
    let rescan: &NSView = &panel.rescan;
    for sibling in &panel.view.subviews() {
        if std::ptr::eq(Retained::as_ptr(&sibling), rescan) {
            continue;
        }
        checks.expect(
            !overlaps(frame, sibling.frame()),
            format!("the button {frame:?} overlaps {:?}", sibling.frame()),
        );
    }
}

fn menu_item(marker: MainThreadMarker, checks: &mut Checks) {
    let menu = main_menu(marker);
    let application = menu
        .itemAtIndex(0)
        .and_then(|item| item.submenu())
        .expect("the application menu comes first");
    let Some(item) = application.itemWithTitle(&NSString::from_str(strings::RESCAN_DISK)) else {
        checks.expect(false, "the application menu offers the rescan");
        return;
    };
    let key = item.keyEquivalent().to_string();
    checks.expect(key == RESCAN_KEY, format!("the rescan answers ⌘{key}"));
    checks.expect(
        item.action() == Some(sel!(rescanDisk:)),
        "the menu item sends rescanDisk:",
    );
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the sidebar check runs on the main thread");
    let _application = NSApplication::sharedApplication(marker);
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let data = fixture_data();
    let actions = ShellActions::new(marker);
    let mut checks = Checks::default();
    button_contract(marker, &data, &actions, &mut checks);
    follows_the_gate(marker, &data, &actions, &mut checks);
    sits_in_the_disk_block(marker, &data, &actions, &mut checks);
    menu_item(marker, &mut checks);
    if checks.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "the sidebar rescan breaks its contract:\n{}",
            checks.failures.join("\n")
        );
        ExitCode::FAILURE
    }
}
