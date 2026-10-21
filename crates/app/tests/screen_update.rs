use std::path::{Path, PathBuf};
use std::process::ExitCode;

use domovoi_app::{
    DuplicatesModel, DuplicatesScreen, DuplicatesState, MainScreen, OutlineFixture, RecommendationsScreen, Screen,
    ScreenKey, Section, ShellActions, ShellData, ShellFixture, ShellPalette, WINDOW, development_fonts,
};
use domovoi_render::{DiagramMode, ROOT, register_fonts};
use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSBackingStoreType, NSScrollView, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect};

const COPIES: usize = 8;
const SCROLLED: f64 = 600.0;
const GROWTH: u64 = 1_000_000_000;
const OFFSET_TOLERANCE: f64 = 0.5;
const NOTES: &str = "Notes";
const NOTE_FILES: usize = 30;
const ARCHIVE: &str = "Archive";
const LIST_SCROLLED: f64 = 200.0;
const LONG_FREE_BYTES: u64 = 987_654_321_000;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn shell_data(screen: Screen) -> ShellData {
    ShellFixture::read(&fixture())
        .and_then(|fixture| fixture.shell_data(screen))
        .expect("the fixture builds the screen data")
}

fn note_files(count: usize, gigabytes: f64) -> Vec<OutlineFixture> {
    (0..count)
        .map(|number| OutlineFixture {
            name: format!("note-{number:02}.txt"),
            category: "home".to_owned(),
            gigabytes: Some(gigabytes + number as f64 / 10.0),
            children: Vec::new(),
        })
        .collect()
}

fn disk_data(files: usize, gigabytes: f64, archive: bool) -> ShellData {
    let mut fixture = ShellFixture::read(&fixture()).expect("the fixture reads");
    if archive {
        fixture.tree.children.push(OutlineFixture {
            name: ARCHIVE.to_owned(),
            category: "home".to_owned(),
            gigabytes: None,
            children: note_files(2, 100.0),
        });
    }
    fixture.tree.children.push(OutlineFixture {
        name: NOTES.to_owned(),
        category: "home".to_owned(),
        gigabytes: None,
        children: note_files(files, gigabytes),
    });
    fixture
        .shell_data(Screen::Main)
        .expect("the fixture builds the disk data")
}

fn notes_position(data: &ShellData) -> usize {
    data.tree
        .node(ROOT)
        .children
        .iter()
        .copied()
        .find(|child| data.tree.node(*child).name == NOTES)
        .expect("the notes folder is in the tree")
}

fn palette(tokens: &'static Palette) -> ShellPalette {
    ShellPalette {
        tokens,
        high_contrast: false,
    }
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

fn offset(scroll: &NSScrollView) -> f64 {
    scroll.contentView().bounds().origin.y
}

fn stays_in(scroll: &NSScrollView, window: &NSWindow) -> bool {
    scroll
        .window()
        .is_some_and(|shown| std::ptr::eq(Retained::as_ptr(&shown), window))
}

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

fn recommendations_keep_their_place(marker: MainThreadMarker, actions: &ShellActions, checks: &mut Checks) {
    let mut data = shell_data(Screen::Recommendations);
    let cards = data.care.recommendations.cards.clone();
    data.care.recommendations.cards = (0..COPIES).flat_map(|_| cards.iter().cloned()).collect();
    let window = offscreen_window(marker);
    let screen = RecommendationsScreen::build(marker, &data, &palette(&Palette::DARK), actions);
    window.setContentView(Some(&screen.frame.root()));
    let (collection, _) = screen.collection.as_ref().expect("the fixture has cards");
    let scroll = collection.enclosingScrollView().expect("the cards scroll");
    lay_out(&window, &scroll);
    scroll_to(&scroll, SCROLLED);
    let before = offset(&scroll);
    let cards_before = collection.numberOfItemsInSection(0);
    let mut changed = data.clone();
    let model = &mut changed.care.recommendations;
    for card in &mut model.cards {
        card.amount += GROWTH;
    }
    let extra = model.cards[0].clone();
    model.cards.push(extra);
    model.safe += GROWTH;
    screen.update(&changed);
    lay_out(&window, &scroll);
    checks.expect(
        (before - SCROLLED).abs() < OFFSET_TOLERANCE,
        format!("recommendations: the long list scrolls to {SCROLLED} pt (got {before})"),
    );
    let same_scroll = collection
        .enclosingScrollView()
        .is_some_and(|now| std::ptr::eq(Retained::as_ptr(&now), Retained::as_ptr(&scroll)));
    checks.expect(
        same_scroll && stays_in(&scroll, &window),
        "recommendations: the grid keeps its scroll view in the window",
    );
    let after = offset(&scroll);
    checks.expect(
        (after - before).abs() < OFFSET_TOLERANCE,
        format!("recommendations: the offset stays at {before} pt (got {after})"),
    );
    checks.expect(
        collection.numberOfItemsInSection(0) == cards_before + 1,
        "recommendations: the new card is in the grid",
    );
    checks.expect(
        screen.total.stringValue().to_string() == changed.care.recommendations.total_line(),
        "recommendations: the total line shows the new total",
    );
}

fn duplicates_keep_their_place(marker: MainThreadMarker, actions: &ShellActions, checks: &mut Checks) {
    let mut data = shell_data(Screen::Duplicates);
    let DuplicatesState::Found { groups, .. } = &mut data.care.duplicates.state else {
        panic!("the fixture has found duplicates");
    };
    let original = groups.clone();
    *groups = (0..COPIES).flat_map(|_| original.iter().cloned()).collect();
    let window = offscreen_window(marker);
    let screen = DuplicatesScreen::build(marker, &data, &palette(&Palette::DARK), actions);
    window.setContentView(Some(&screen.frame.root()));
    let (outline, _) = screen.outline.as_ref().expect("the fixture has groups");
    let scroll = outline.enclosingScrollView().expect("the groups scroll");
    lay_out(&window, &scroll);
    scroll_to(&scroll, SCROLLED);
    let before = offset(&scroll);
    let mut changed = data.clone();
    let DuplicatesState::Found { groups, .. } = &mut changed.care.duplicates.state else {
        panic!("the changed data keeps the found duplicates");
    };
    for copy in groups.iter_mut().flat_map(|group| group.copies.iter_mut()) {
        copy.frees += GROWTH;
    }
    let extra = groups[0].clone();
    groups.push(extra);
    let expected_rows: usize = groups.iter().map(|group| group.copies.len() + 2).sum();
    screen.update(&changed);
    lay_out(&window, &scroll);
    checks.expect(
        (before - SCROLLED).abs() < OFFSET_TOLERANCE,
        format!("duplicates: the long list scrolls to {SCROLLED} pt (got {before})"),
    );
    let same_scroll = outline
        .enclosingScrollView()
        .is_some_and(|now| std::ptr::eq(Retained::as_ptr(&now), Retained::as_ptr(&scroll)));
    checks.expect(
        same_scroll && stays_in(&scroll, &window),
        "duplicates: the outline keeps its scroll view in the window",
    );
    let after = offset(&scroll);
    checks.expect(
        (after - before).abs() < OFFSET_TOLERANCE,
        format!("duplicates: the offset stays at {before} pt (got {after})"),
    );
    let rows = usize::try_from(outline.numberOfRows()).expect("a row count is non-negative");
    checks.expect(
        rows == expected_rows,
        format!("duplicates: every group is expanded with the new one ({rows} rows, expected {expected_rows})"),
    );
    checks.expect(
        changed
            .care
            .duplicates
            .title_line()
            .is_some_and(|title| screen.title.stringValue().to_string() == title),
        "duplicates: the title shows the new total",
    );
}

fn sidebar_follows_the_data(marker: MainThreadMarker, actions: &ShellActions, checks: &mut Checks) {
    let data = shell_data(Screen::Recommendations);
    let window = offscreen_window(marker);
    let screen = RecommendationsScreen::build(marker, &data, &palette(&Palette::DARK), actions);
    window.setContentView(Some(&screen.frame.root()));
    let mut changed = data.clone();
    changed.free_bytes += GROWTH;
    changed.rescan_allowed = !data.rescan_allowed;
    screen.update(&changed);
    let sidebar = &screen.frame.sidebar;
    checks.expect(
        sidebar.free.stringValue().to_string() == changed.free_line(),
        "sidebar: the free line shows the new free space",
    );
    checks.expect(
        sidebar.rescan.isEnabled() == changed.rescan_allowed,
        "sidebar: the rescan button follows the gate",
    );
    let outline_scroll = sidebar.outline.enclosingScrollView().expect("the sidebar list scrolls");
    checks.expect(
        stays_in(&outline_scroll, &window),
        "sidebar: the list keeps its views in the window",
    );
}

fn disk_keeps_its_place(marker: MainThreadMarker, actions: &ShellActions, checks: &mut Checks) {
    let data = disk_data(NOTE_FILES, 0.5, false);
    let window = offscreen_window(marker);
    let mut screen = MainScreen::build(
        marker,
        &data,
        notes_position(&data),
        DiagramMode::Sunburst,
        &palette(&Palette::DARK),
        actions,
    );
    window.setContentView(Some(&screen.frame.root()));
    let scroll = screen.list.scroll.clone();
    lay_out(&window, &scroll);
    scroll_to(&scroll, LIST_SCROLLED);
    let before = offset(&scroll);
    let mut changed = disk_data(NOTE_FILES + 1, 1.5, true);
    changed.free_bytes = LONG_FREE_BYTES;
    let moved_focus = notes_position(&changed);
    screen.update(&changed, moved_focus);
    lay_out(&window, &scroll);
    checks.expect(
        (before - LIST_SCROLLED).abs() < OFFSET_TOLERANCE,
        format!("disk: the level list scrolls to {LIST_SCROLLED} pt (got {before})"),
    );
    let same_scroll = screen
        .list
        .table
        .enclosingScrollView()
        .is_some_and(|now| std::ptr::eq(Retained::as_ptr(&now), Retained::as_ptr(&scroll)));
    checks.expect(
        same_scroll && stays_in(&scroll, &window),
        "disk: the level list keeps its scroll view in the window",
    );
    let after = offset(&scroll);
    checks.expect(
        (after - before).abs() < OFFSET_TOLERANCE,
        format!("disk: the offset stays at {before} pt (got {after})"),
    );
    let rows = usize::try_from(screen.list.table.numberOfRows()).expect("a row count is non-negative");
    checks.expect(
        rows == NOTE_FILES + 1,
        format!("disk: the new row is in the list ({rows} rows)"),
    );
    checks.expect(
        moved_focus != notes_position(&data),
        "disk: the folder moved in the tree, so the crumbs need new nodes",
    );
    let current_crumb = screen.crumbs.buttons().last().map(|button| button.tag());
    checks.expect(
        current_crumb == isize::try_from(moved_focus).ok(),
        format!("disk: the current crumb points at the moved folder ({current_crumb:?} vs {moved_focus})"),
    );
    checks.expect(
        screen.free.stringValue().to_string() == changed.free_short(),
        "disk: the head shows the new free space",
    );
    let crumbs = screen.crumbs.frame();
    let switch = screen.modes.view.frame();
    let free = screen.free.frame();
    checks.expect(
        crumbs.origin.x + crumbs.size.width <= switch.origin.x,
        format!("disk: the crumbs {crumbs:?} end before the switch {switch:?}"),
    );
    checks.expect(
        switch.origin.x + switch.size.width <= free.origin.x,
        format!("disk: the switch {switch:?} ends before the longer free figure {free:?}"),
    );
}

fn key(section: Section, palette: ShellPalette, data: &ShellData) -> Option<ScreenKey> {
    ScreenKey::of(section, palette, data, ROOT, DiagramMode::Sunburst)
}

fn disk_keys_follow_the_level(checks: &mut Checks) {
    let dark = palette(&Palette::DARK);
    let disk = disk_data(NOTE_FILES, 0.5, false);
    let notes = notes_position(&disk);
    let shown = ScreenKey::of(Section::Disk, dark, &disk, notes, DiagramMode::Sunburst);
    checks.expect(shown.is_some(), "keys: the disk has a key");
    checks.expect(
        shown != ScreenKey::of(Section::Disk, dark, &disk, ROOT, DiagramMode::Sunburst),
        "keys: another focus is another key",
    );
    checks.expect(
        shown != ScreenKey::of(Section::Disk, dark, &disk, notes, DiagramMode::Treemap),
        "keys: another mode is another key",
    );
    let mut cloud = disk.clone();
    cloud.in_cloud_bytes = if disk.in_cloud_bytes > 0 { 0 } else { GROWTH };
    checks.expect(
        shown != ScreenKey::of(Section::Disk, dark, &cloud, notes, DiagramMode::Sunburst),
        "keys: the cloud figure appearing is another key",
    );
    let grown = disk_data(NOTE_FILES + 1, 1.5, true);
    checks.expect(
        shown
            == ScreenKey::of(
                Section::Disk,
                dark,
                &grown,
                notes_position(&grown),
                DiagramMode::Sunburst,
            ),
        "keys: the same folder with other sizes and positions keeps the key",
    );
}

fn keys_follow_the_shape(checks: &mut Checks) {
    let dark = palette(&Palette::DARK);
    let light = palette(&Palette::LIGHT);
    let recommendations = shell_data(Screen::Recommendations);
    let shown = key(Section::Recommendations, dark, &recommendations);
    checks.expect(shown.is_some(), "keys: the recommendations have a key");
    checks.expect(
        shown != key(Section::Duplicates, dark, &recommendations),
        "keys: another section is another key",
    );
    checks.expect(
        shown != key(Section::Recommendations, light, &recommendations),
        "keys: another palette is another key",
    );
    let mut no_cards = recommendations.clone();
    no_cards.care.recommendations.cards.clear();
    checks.expect(
        shown != key(Section::Recommendations, dark, &no_cards),
        "keys: no cards is another key",
    );
    let mut grown = recommendations.clone();
    for card in &mut grown.care.recommendations.cards {
        card.amount += GROWTH;
    }
    grown.free_bytes += GROWTH;
    checks.expect(
        shown == key(Section::Recommendations, dark, &grown),
        "keys: other sizes keep the key",
    );
    let duplicates = shell_data(Screen::Duplicates);
    let found = key(Section::Duplicates, dark, &duplicates);
    let mut idle = duplicates.clone();
    idle.care.duplicates = DuplicatesModel::idle();
    checks.expect(
        found != key(Section::Duplicates, dark, &idle),
        "keys: another duplicates body is another key",
    );
    let mut cloning = duplicates.clone();
    cloning.care.duplicates.clone_mode = true;
    checks.expect(
        found != key(Section::Duplicates, dark, &cloning),
        "keys: the clone mode is another key",
    );
    for section in [Section::Assistant, Section::Model] {
        checks.expect(
            key(section, dark, &recommendations).is_none(),
            format!("keys: {section:?} has no key yet"),
        );
    }
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the update check runs on the main thread");
    let _application = NSApplication::sharedApplication(marker);
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let actions = ShellActions::new(marker);
    let mut checks = Checks::default();
    recommendations_keep_their_place(marker, &actions, &mut checks);
    duplicates_keep_their_place(marker, &actions, &mut checks);
    sidebar_follows_the_data(marker, &actions, &mut checks);
    disk_keeps_its_place(marker, &actions, &mut checks);
    keys_follow_the_shape(&mut checks);
    disk_keys_follow_the_level(&mut checks);
    if checks.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!("screens lose their place on update:\n{}", checks.failures.join("\n"));
        ExitCode::FAILURE
    }
}
