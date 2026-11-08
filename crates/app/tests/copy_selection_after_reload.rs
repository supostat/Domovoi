use std::path::{Path, PathBuf};
use std::process::ExitCode;

use domovoi_app::{
    DuplicatesScreen, DuplicatesState, Screen, ShellActions, ShellData, ShellFixture, ShellPalette, WINDOW,
    development_fonts,
};
use domovoi_render::register_fonts;
use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSBackingStoreType, NSButton, NSControlStateValueOff, NSControlStateValueOn,
    NSScrollView, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect};

const COPIES: usize = 8;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn long_list() -> ShellData {
    let mut data = ShellFixture::read(&fixture())
        .and_then(|fixture| fixture.shell_data(Screen::Duplicates))
        .expect("the fixture builds the duplicates screen data");
    let DuplicatesState::Found { groups, .. } = &mut data.care.duplicates.state else {
        panic!("the fixture has found duplicates");
    };
    let original = groups.clone();
    *groups = (0..COPIES)
        .flat_map(|round| {
            original.iter().cloned().map(move |mut group| {
                for member in std::iter::once(&mut group.keep).chain(&mut group.copies) {
                    member.path = format!("{}-{round}", member.path);
                }
                group
            })
        })
        .collect();
    data
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

fn lay_out(window: &NSWindow, scroll: &NSScrollView) {
    scroll.tile();
    window
        .contentView()
        .expect("the window shows the screen")
        .layoutSubtreeIfNeeded();
    if let Some(document) = scroll.documentView() {
        document.layoutSubtreeIfNeeded();
    }
}

fn find_check_box(view: &NSView, label: &str) -> Option<Retained<NSButton>> {
    view.subviews().iter().find_map(|child| {
        let named = child.accessibilityLabel().is_some_and(|text| text.to_string() == label);
        match child.clone().downcast::<NSButton>() {
            Ok(button) if named => Some(button),
            _ => find_check_box(&child, label),
        }
    })
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the selection check runs on the main thread");
    let _application = NSApplication::sharedApplication(marker);
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let actions = ShellActions::new(marker);
    let mut data = long_list();
    let model = &mut data.care.duplicates;
    let clone_mode = model.clone_mode;
    let (group, copy) = model
        .groups()
        .iter()
        .enumerate()
        .find_map(|(group, model)| {
            model
                .copies
                .iter()
                .position(|copy| copy.can_check(clone_mode))
                .map(|copy| (group, copy))
        })
        .expect("the fixture has a copy that can be checked");
    let label = model.groups()[group].copies[copy].path.clone();
    let checked = !model.groups()[group].copies[copy].checked;
    let window = offscreen_window(marker);
    let palette = ShellPalette {
        tokens: &Palette::DARK,
        high_contrast: false,
    };
    let screen = DuplicatesScreen::build(marker, &data, &palette, &actions);
    window.setContentView(Some(&screen.frame.root()));
    let (outline, _) = screen.outline.as_ref().expect("the fixture has groups");
    let scroll = outline.enclosingScrollView().expect("the groups scroll");
    lay_out(&window, &scroll);

    let check_box = find_check_box(outline, &label).expect("the copy is on screen");
    unsafe { check_box.performClick(None) };
    data.care.duplicates.groups_mut()[group].copies[copy].checked = checked;
    screen.show_selection(&data.care.duplicates);
    outline.reloadData();
    lay_out(&window, &scroll);

    let expected = if checked {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    };
    let state = find_check_box(outline, &label)
        .expect("the copy is on screen again")
        .state();
    if state == expected {
        println!("ok   a copy row built again shows the check the user gave it");
        ExitCode::SUCCESS
    } else {
        eprintln!("FAIL a copy row built again shows state {state}, the model says {expected}");
        ExitCode::FAILURE
    }
}
