use std::path::{Path, PathBuf};
use std::process::ExitCode;

use domovoi_app::{
    RecommendationsScreen, Screen, ShellActions, ShellData, ShellFixture, ShellPalette, WINDOW, development_fonts,
};
use domovoi_render::register_fonts;
use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSBackingStoreType, NSButton, NSCollectionView, NSControlStateValueOff,
    NSScrollView, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect};

const COPIES: usize = 8;
const FIRST: usize = 0;

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

fn scroll_to(scroll: &NSScrollView, top: f64) {
    let clip = scroll.contentView();
    clip.scrollToPoint(NSPoint::new(0.0, top));
    scroll.reflectScrolledClipView(&clip);
}

fn check_box_of(collection: &NSCollectionView, position: usize, title: &str) -> Retained<NSButton> {
    let item = collection.itemAtIndex(position).expect("the card is on screen");
    find_check_box(&item.view(), title).expect("the card has a check box named after its title")
}

fn find_check_box(view: &NSView, title: &str) -> Option<Retained<NSButton>> {
    view.subviews().iter().find_map(|child| {
        let named = child
            .accessibilityLabel()
            .is_some_and(|label| label.to_string() == title);
        match child.clone().downcast::<NSButton>() {
            Ok(button) if named => Some(button),
            _ => find_check_box(&child, title),
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
    let card = &data.care.recommendations.cards[FIRST];
    assert!(
        card.selected && card.rows.is_empty(),
        "the first card of the fixture is a checked card without rows"
    );
    let title = card.title.clone();
    let window = offscreen_window(marker);
    let palette = ShellPalette {
        tokens: &Palette::DARK,
        high_contrast: false,
    };
    let screen = RecommendationsScreen::build(marker, &data, &palette, &actions);
    window.setContentView(Some(&screen.frame.root()));
    let (collection, _) = screen.collection.as_ref().expect("the fixture has cards");
    let scroll = collection.enclosingScrollView().expect("the cards scroll");
    lay_out(&window, &scroll);

    unsafe { check_box_of(collection, FIRST, &title).performClick(None) };
    data.care.recommendations.cards[FIRST].selected = false;
    screen.show_selection(&data.care.recommendations);
    let bottom = scroll
        .documentView()
        .expect("the cards have a document")
        .frame()
        .size
        .height;
    scroll_to(&scroll, bottom);
    lay_out(&window, &scroll);
    scroll_to(&scroll, 0.0);
    lay_out(&window, &scroll);

    let state = check_box_of(collection, FIRST, &title).state();
    if state == NSControlStateValueOff {
        println!("ok   an unchecked card stays unchecked after scrolling away and back");
        ExitCode::SUCCESS
    } else {
        eprintln!("FAIL an unchecked card is checked again after scrolling away and back (state {state})");
        ExitCode::FAILURE
    }
}
