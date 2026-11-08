use std::path::Path;
use std::process::ExitCode;

use domovoi_app::{
    Breadcrumbs, CardModel, Crumb, DeleteStatus, DropHover, DropKind, HoverPanel, HoverTip, LevelRow, LevelRowView,
    MainScreen, MemberModel, NARROW_WINDOW, ProjectRow, Provenance, PurgeFinish, PurgeModel, PurgeProgress,
    PurgeReport, PurgeScreen, RecommendationCard, SafetyBadge, ScanningScreen, Screen, Section, ShellActions,
    ShellFixture, ShellPalette, SidebarEntry, SidebarItemView, TrayModel, TrayRow, TrayScreen, TraySummary,
    development_fonts, member_row, strings,
};
use domovoi_index::Category;
use domovoi_render::{DiagramMode, register_fonts};
use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAccessibility, NSControl, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

const ROW_HEIGHT: f64 = 30.0;
const NARROW_ROW: f64 = 180.0;
const NARROW_MARKED_ROW: f64 = 240.0;
const SIDEBAR_WIDTH: f64 = 230.0;
const SIDEBAR_ROW: f64 = 28.0;
const LONG_NAME: &str = "Очень длинное имя папки проекта, которое не помещается в строку списка уровня";
const LONG_PATH: &str = "/Users/igor/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw/very/deep/nested/folder/structure/with/a/final/file.bin";
const SIZE_TEXT: &str = "393,2 ГБ";
const CARD_WIDTH: f64 = 520.0;
const DUPLICATE_ROW: f64 = 480.0;
const TITLE_LINES: isize = 2;
const WIDE_CRUMBS: f64 = 2_000.0;
const NARROWEST_CRUMBS: f64 = 40.0;
const CRUMB_STEP: f64 = 10.0;
const CRUMB_HEIGHT: f64 = 30.0;
const ELLIPSIS: &str = "…";
const IN_CLOUD_BYTES: u64 = 12_000_000_000;
const PURGE_ESTIMATE: u64 = 8_400_000_000;
const PURGE_FREED: u64 = 1_200_000_000;
const PURGE_AVAILABLE: u64 = 10_000_000_000;
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

    fn wraps_whole(&mut self, field: &NSTextField, element: &str) {
        let shown = field.frame().size;
        let wrap_width = field.preferredMaxLayoutWidth();
        let needed = field.fittingSize().height;
        self.expect(
            wrap_width > 0.0 && wrap_width <= shown.width + 0.5 && shown.height + 0.5 >= needed,
            format!(
                "{element}: wraps at {wrap_width} pt in {} pt, {} pt tall of {needed} pt",
                shown.width, shown.height
            ),
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
        draggable: true,
        marked: false,
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
        checks.expect(
            parts.mark.isHidden(),
            format!("{element}: an unmarked row hides the mark"),
        );
    }
}

fn marked_level_rows(checks: &mut Checks, marker: MainThreadMarker) {
    let row = LevelRow {
        node: 1,
        name: LONG_NAME.to_owned(),
        size_text: SIZE_TEXT.to_owned(),
        share: 1.0,
        category: Category::Dev,
        draggable: true,
        marked: true,
    };
    for width in [360.0, NARROW_MARKED_ROW] {
        let host = container(marker, width, ROW_HEIGHT);
        let parts = LevelRowView::build(marker, &host, &row, &palette());
        let element = format!("marked level row at {width} pt");
        checks.expect(!parts.mark.isHidden(), format!("{element}: the mark is shown"));
        checks.expect(
            parts.name.frame().size.width < parts.name.fittingSize().width,
            format!("{element}: the long name is shortened first"),
        );
        checks.mode(&parts.mark, NSLineBreakMode::ByClipping, &format!("{element}, mark"));
        checks.never_cut(&parts.mark, &format!("{element}, mark"));
        checks.never_cut(&parts.size, &format!("{element}, size"));
        checks.expect(
            parts.name.frame().origin.x + parts.name.frame().size.width <= parts.mark.frame().origin.x,
            format!("{element}: the name stops before the mark"),
        );
        checks.expect(
            parts.mark.frame().origin.x + parts.mark.frame().size.width <= parts.size.frame().origin.x,
            format!("{element}: the mark stops before the size"),
        );
        let tooltip = parts.name.toolTip().map(|tooltip| tooltip.to_string());
        checks.expect(
            tooltip.as_deref() == Some(LONG_NAME),
            format!("{element}: tooltip {tooltip:?}"),
        );
        let label = parts.name.accessibilityLabel().map(|label| label.to_string());
        let spoken = format!("{LONG_NAME}, {}", strings::MARKED);
        checks.expect(
            label.as_deref() == Some(spoken.as_str()),
            format!("{element}: VoiceOver label {label:?}"),
        );
    }
}

fn sidebar(checks: &mut Checks, marker: MainThreadMarker) {
    let entry = SidebarEntry {
        title: LONG_NAME.to_owned(),
        counter: Some(SIZE_TEXT.to_owned()),
        children: Vec::new(),
        section: Some(Section::Recommendations),
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

fn titles(crumbs: &Breadcrumbs) -> Vec<String> {
    crumbs
        .buttons()
        .iter()
        .map(|button| button.title().to_string())
        .collect()
}

fn right_edge(view: &NSView) -> f64 {
    view.subviews()
        .iter()
        .map(|child| child.frame().origin.x + child.frame().size.width)
        .fold(0.0, f64::max)
}

fn crumb_path() -> Vec<Crumb> {
    CRUMBS
        .iter()
        .enumerate()
        .map(|(node, title)| Crumb {
            node,
            title: (*title).to_owned(),
        })
        .collect()
}

fn crumbs(checks: &mut Checks, marker: MainThreadMarker) {
    let actions = ShellActions::new(marker);
    let crumbs = Breadcrumbs::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(WIDE_CRUMBS, CRUMB_HEIGHT)),
        &crumb_path(),
        &palette(),
        &actions,
    );
    let all = titles(&crumbs);
    checks.expect(all == CRUMBS, format!("a wide frame shows every level: {all:?}"));
    checks.expect(crumbs.collapsed().is_empty(), "nothing is collapsed on a wide frame");
    for (button, title) in crumbs.buttons().iter().zip(CRUMBS) {
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
    let full = right_edge(&crumbs);
    let mut shown_levels = CRUMBS.len();
    let mut width = full;
    while width >= NARROWEST_CRUMBS {
        crumbs.setFrameSize(NSSize::new(width, CRUMB_HEIGHT));
        shown_levels = crumbs_step(checks, &crumbs, width, shown_levels);
        width -= CRUMB_STEP;
    }
    crumbs.setFrameSize(NSSize::new(NARROWEST_CRUMBS, CRUMB_HEIGHT));
    let narrowest = titles(&crumbs);
    let current = CRUMBS[CRUMBS.len() - 1];
    checks.expect(
        narrowest == [ELLIPSIS, current],
        format!("the narrowest frame keeps the ellipsis and the current level: {narrowest:?}"),
    );
    let buttons = crumbs.buttons();
    let last = buttons.last().expect("the current level stays");
    checks.expect(
        last.frame().size.width < last.fittingSize().width,
        "the current level is cut on the narrowest frame",
    );
    checks.truncated_with_full_text(last, current, "cut current crumb");
    crumbs.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(full - 1.0, CRUMB_HEIGHT)));
    checks.expect(
        titles(&crumbs) != CRUMBS,
        "a frame change through setFrame: collapses the crumbs too",
    );
    crumbs.setFrameSize(NSSize::new(WIDE_CRUMBS, CRUMB_HEIGHT));
    let regrown = titles(&crumbs);
    checks.expect(
        regrown == CRUMBS,
        format!("widening brings every level back: {regrown:?}"),
    );
}

fn crumbs_step(checks: &mut Checks, crumbs: &Breadcrumbs, width: f64, shown_before: usize) -> usize {
    let shown = titles(crumbs);
    let element = format!("crumbs at {width} pt {shown:?}");
    checks.expect(right_edge(crumbs) <= width + 0.5, format!("{element}: overflow"));
    let all = shown == CRUMBS;
    let first = !all && shown.first().is_some_and(|title| title == CRUMBS[0]);
    let mut rest: &[String] = &shown;
    if first {
        rest = &rest[1..];
    }
    let ellipsis = rest.first().is_some_and(|title| title == ELLIPSIS);
    if ellipsis {
        rest = &rest[1..];
    }
    let tail = &CRUMBS[CRUMBS.len() - rest.len()..];
    checks.expect(rest == tail, format!("{element}: the tail runs to the current level"));
    let expected: Vec<&str> = if all {
        Vec::new()
    } else {
        CRUMBS[usize::from(first)..CRUMBS.len() - rest.len()].to_vec()
    };
    let collapsed: Vec<String> = crumbs.collapsed().into_iter().map(|crumb| crumb.title).collect();
    checks.expect(collapsed == expected, format!("{element}: collapsed {collapsed:?}"));
    checks.expect(
        ellipsis != expected.is_empty(),
        format!("{element}: ellipsis only for collapsed levels"),
    );
    let buttons = crumbs.buttons();
    if ellipsis {
        let menu = buttons[usize::from(first)].menu().expect("the ellipsis carries a menu");
        let entries: Vec<String> = menu.itemArray().iter().map(|item| item.title().to_string()).collect();
        checks.expect(
            entries == expected,
            format!("{element}: the ellipsis menu lists {entries:?}"),
        );
    }
    for button in &buttons[..buttons.len() - 1] {
        checks.expect(
            button.frame().size.width + 0.5 >= button.fittingSize().width,
            format!("{element}: {} is cut", button.title()),
        );
    }
    let levels = shown.len() - usize::from(ellipsis);
    checks.expect(
        levels <= shown_before,
        format!("{element}: a narrower frame shows more levels"),
    );
    levels
}

fn head(checks: &mut Checks, marker: MainThreadMarker) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json");
    let mut data = ShellFixture::read(&fixture)
        .and_then(|fixture| fixture.shell_data(Screen::Main))
        .expect("the fixture reads");
    data.in_cloud_bytes = IN_CLOUD_BYTES;
    let deepest = (0..data.tree.len())
        .max_by_key(|node| data.crumbs(*node).len())
        .expect("the fixture tree has nodes");
    let actions = ShellActions::new(marker);
    let screen = MainScreen::build(marker, &data, deepest, DiagramMode::Sunburst, &palette(), &actions);
    checks.expect(screen.in_cloud.is_some(), "the head shows the cloud figure");
    let crumbs = screen.crumbs.frame();
    let switch = screen.modes.view.frame();
    checks.expect(
        crumbs.origin.x + crumbs.size.width <= switch.origin.x,
        format!("the crumbs {crumbs:?} end before the switch {switch:?}"),
    );
    checks.expect(
        right_edge(&screen.crumbs) <= crumbs.size.width + 0.5,
        format!("the crumbs fit their frame: {:?}", titles(&screen.crumbs)),
    );
    let longest = TraySummary {
        objects: 12_345,
        bytes: 1_234_500_000_000,
    };
    screen.show_tray_summary(&longest.title());
    head_in_order(checks, &screen, "a long tray summary");
    screen.arm_tray(DropHover {
        kind: DropKind::Add,
        title: format!("{} 12 345 из 12 346 · ≈ 1 234,5 ГБ", strings::DROP_ADD),
    });
    head_in_order(checks, &screen, "a drag over a long tray summary");
}

fn head_in_order(checks: &mut Checks, screen: &MainScreen, case: &str) {
    let cloud = screen.in_cloud.as_ref().expect("the head shows the cloud figure");
    let frames = [
        ("crumbs", screen.crumbs.frame()),
        ("switch", screen.modes.view.frame()),
        ("cloud", cloud.frame()),
        ("free", screen.free.frame()),
        ("tray", screen.tray.body_frame()),
    ];
    for pair in frames.windows(2) {
        let ((left_name, left), (right_name, right)) = (pair[0], pair[1]);
        checks.expect(
            left.origin.x + left.size.width <= right.origin.x,
            format!("{case}: the {left_name} {left:?} runs into the {right_name} {right:?}"),
        );
    }
    let crumbs = screen.crumbs.frame();
    checks.expect(
        right_edge(&screen.crumbs) <= crumbs.size.width + 0.5,
        format!(
            "{case}: the crumbs fit their shrunk frame: {:?}",
            titles(&screen.crumbs)
        ),
    );
}

fn scanning(checks: &mut Checks, marker: MainThreadMarker) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json");
    let mut data = ShellFixture::read(&fixture)
        .and_then(|fixture| fixture.shell_data(Screen::Main))
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
            note: Some(strings::SNAPSHOTS_RELEASE.to_owned()),
        },
        &palette(),
    );
    checks.mode(&panel.path, NSLineBreakMode::ByTruncatingMiddle, "hover path");
    checks.truncated_with_full_text(&panel.path, LONG_PATH, "hover path");
    checks.mode(&panel.name, NSLineBreakMode::ByTruncatingTail, "hover name");
    checks.truncated_with_full_text(&panel.name, LONG_NAME, "hover name");
    checks.never_cut(&panel.size, "hover size");
    checks.mode(&panel.note, NSLineBreakMode::ByWordWrapping, "hover note");
}

fn project_rows(checks: &mut Checks, marker: MainThreadMarker) {
    let row = |stale: bool| ProjectRow {
        name: LONG_NAME.to_owned(),
        path: LONG_PATH.to_owned(),
        artifacts: vec![
            "node_modules ×3".to_owned(),
            ".next".to_owned(),
            ".turbo".to_owned(),
            "__pycache__".to_owned(),
        ],
        age: "11 месяцев назад".to_owned(),
        stale,
        amount: 393_200_000_000,
        selected: stale,
        targets: Vec::new(),
    };
    let card = CardModel {
        title: strings::PROJECTS_TITLE.to_owned(),
        paths: vec![LONG_PATH.to_owned()],
        path_reasons: Vec::new(),
        explanation: strings::PROJECTS_EXPLANATION.to_owned(),
        level: SafetyBadge::Safe,
        provenance: Provenance::Projects,
        amount: 786_400_000_000,
        held_by_snapshots: 0,
        held_by_clones: 0,
        held_by_hard_links: 0,
        owner_running: None,
        blocked: None,
        vendor_command: None,
        age: None,
        selected: false,
        rows: vec![row(true), row(false)],
        targets: Vec::new(),
    };
    let actions = ShellActions::new(marker);
    let view = RecommendationCard::build(marker, &card, 0, CARD_WIDTH, &palette(), &actions, true);
    checks.expect(view.rows.len() == card.rows.len(), "every project has its row");
    checks.never_cut(&view.amount, "projects card selected amount");
    checks.never_cut(
        view.total.as_ref().expect("a projects card shows its whole amount"),
        "projects card total",
    );
    for (index, parts) in view.rows.iter().enumerate() {
        let element = format!("project row {index}");
        let source = &card.rows[index];
        checks.mode(
            &parts.name,
            NSLineBreakMode::ByTruncatingMiddle,
            &format!("{element}, name"),
        );
        checks.truncated_with_full_text(&parts.name, &source.name, &format!("{element}, name"));
        checks.mode(
            &parts.path,
            NSLineBreakMode::ByTruncatingMiddle,
            &format!("{element}, path"),
        );
        checks.truncated_with_full_text(&parts.path, &source.path, &format!("{element}, path"));
        checks.truncated_with_full_text(
            &parts.artifacts,
            &source.artifacts.join(" · "),
            &format!("{element}, artifacts"),
        );
        checks.never_cut(&parts.age, &format!("{element}, age"));
        checks.never_cut(&parts.size, &format!("{element}, size"));
        let right = |field: &NSTextField| field.frame().origin.x + field.frame().size.width;
        checks.expect(
            right(&parts.name) <= parts.age.frame().origin.x
                && right(&parts.age) <= parts.size.frame().origin.x
                && right(&parts.path) <= parts.artifacts.frame().origin.x
                && right(&parts.size) <= CARD_WIDTH
                && parts.name.frame().size.width > 0.0
                && parts.path.frame().size.width > 0.0,
            format!("{element}: name, age and size on top, path and artifacts below, side by side"),
        );
    }
}

fn recommendation_card(checks: &mut Checks, marker: MainThreadMarker) {
    let card = CardModel {
        title: format!("{LONG_NAME} {LONG_NAME}"),
        paths: vec![LONG_PATH.to_owned(), LONG_PATH.to_owned()],
        path_reasons: vec![
            Some(LONG_NAME.to_owned()),
            Some(strings::OWNER_CONTAINER_METADATA.to_owned()),
        ],
        explanation: format!("{LONG_NAME}. {LONG_NAME}. {LONG_NAME}."),
        level: SafetyBadge::Safe,
        provenance: Provenance::Rule("cargo.target".to_owned()),
        amount: 393_200_000_000,
        held_by_snapshots: 0,
        held_by_clones: 0,
        held_by_hard_links: 0,
        owner_running: None,
        blocked: None,
        vendor_command: None,
        age: None,
        selected: true,
        rows: Vec::new(),
        targets: Vec::new(),
    };
    let actions = ShellActions::new(marker);
    let view = RecommendationCard::build(marker, &card, 0, CARD_WIDTH, &palette(), &actions, true);
    checks.expect(
        view.title.maximumNumberOfLines() == TITLE_LINES,
        "card title keeps at most two lines",
    );
    checks.expect(
        view.ask.frame().size.width <= CARD_WIDTH && view.ask.title().to_string() == strings::ASK_DOMOVOI,
        "card ask button shows its whole title inside the card",
    );
    checks.expect(
        view.title.frame().size.height < 3.0 * view.title.font().expect("the title has a font").pointSize(),
        "card title is cut to two lines",
    );
    checks.truncated_with_full_text(&view.title, &card.title, "card title");
    let joined = card.paths.join(", ");
    checks.mode(&view.path, NSLineBreakMode::ByTruncatingMiddle, "card path");
    checks.truncated_with_full_text(&view.path, &joined, "card path");
    checks.never_cut(&view.amount, "card amount");
    let reasons = view.reasons.as_ref().expect("a card with reasons shows them");
    checks.mode(reasons, NSLineBreakMode::ByWordWrapping, "card reasons");
    checks.wraps_whole(reasons, "card reasons");
}

fn tray_application_group(checks: &mut Checks, marker: MainThreadMarker) {
    let model = TrayModel {
        rows: vec![TrayRow {
            category: Category::Apps,
            title: format!("{LONG_NAME} {}", strings::WITH_ITS_FILES),
            size: 2_400_000_000,
            details: vec![
                format!("{LONG_PATH} · {}", strings::OWNER_CONTAINER_METADATA),
                format!("{LONG_PATH} · {}", strings::OWNER_LAUNCHD),
            ],
            unmarked: vec![LONG_PATH.to_owned()],
        }],
        ..TrayModel::default()
    };
    let actions = ShellActions::new(marker);
    let screen = TrayScreen::build(marker, &model, &palette(), &actions);
    checks.expect(
        screen.details.len() == 4,
        "the group shows its files, the heading and the unmarked path",
    );
    for (position, field) in screen.details.iter().enumerate() {
        checks.wraps_whole(field, &format!("tray detail {position}"));
    }
}

fn tray_unmark_row(checks: &mut Checks, marker: MainThreadMarker) {
    let model = TrayModel {
        rows: vec![TrayRow {
            category: Category::Dev,
            title: LONG_NAME.to_owned(),
            size: 2_400_000_000,
            details: Vec::new(),
            unmarked: Vec::new(),
        }],
        ..TrayModel::default()
    };
    let actions = ShellActions::new(marker);
    let screen = TrayScreen::build(marker, &model, &palette(), &actions);
    let row = &screen.rows[0];
    let unmark = row.unmark.as_ref().expect("a confirmed row can be unmarked");
    checks.mode(&row.title, NSLineBreakMode::ByTruncatingTail, "tray row title");
    checks.truncated_with_full_text(&row.title, LONG_NAME, "tray row title");
    checks.never_cut(&row.size, "tray row size");
    let title = row.title.frame();
    let size = row.size.frame();
    let cross = unmark.frame();
    checks.expect(
        title.origin.x + title.size.width <= size.origin.x,
        format!("tray row title {title:?} runs into the size {size:?}"),
    );
    checks.expect(
        size.origin.x + size.size.width <= cross.origin.x,
        format!("tray row size {size:?} runs into the unmark cross {cross:?}"),
    );
    let label = unmark.accessibilityLabel().map(|label| label.to_string());
    checks.expect(
        label.as_deref() == Some(format!("{}: {LONG_NAME}", strings::UNMARK).as_str()),
        format!("unmark cross: VoiceOver label {label:?}"),
    );
    let deleting = model.with_deleting(DeleteStatus {
        files_done: 1,
        files_total: 2,
    });
    let screen = TrayScreen::build(marker, &deleting, &palette(), &actions);
    checks.expect(
        screen.rows.iter().all(|row| row.unmark.is_none()),
        "a row being deleted cannot be unmarked",
    );
}

fn purge_sheet(checks: &mut Checks, marker: MainThreadMarker) {
    let report = PurgeReport {
        estimate: PURGE_ESTIMATE,
        freed: PURGE_FREED,
        available_before: PURGE_AVAILABLE,
        available_after: PURGE_AVAILABLE + PURGE_FREED,
        finish: PurgeFinish::TimedOut,
    };
    let states = [
        (
            "confirming",
            PurgeModel::Confirming {
                estimate: PURGE_ESTIMATE,
                time_machine: true,
            },
            strings::PURGE_RELEASE,
        ),
        (
            "releasing",
            PurgeModel::Releasing(PurgeProgress {
                estimate: PURGE_ESTIMATE,
                freed: PURGE_FREED,
            }),
            strings::STOP,
        ),
        ("released", PurgeModel::Released(Ok(report)), strings::CLOSE),
        (
            "refused",
            PurgeModel::Released(Err(LONG_PATH.to_owned())),
            strings::CLOSE,
        ),
    ];
    let actions = ShellActions::new(marker);
    for (name, model, primary) in states {
        let screen = PurgeScreen::build(marker, &model, &palette(), &actions);
        let element = format!("purge sheet {name}");
        let title = screen.primary.title().to_string();
        checks.expect(title == primary, format!("{element}: primary button {title}"));
        checks.expect(
            screen.probes.accent_button.is_some(),
            format!("{element}: the primary button is probed"),
        );
        checks.expect(
            (screen.size().width - NARROW_WINDOW.width).abs() < 0.5,
            format!("{element}: the sheet keeps the narrow width"),
        );
        checks.expect(!screen.lines.is_empty(), format!("{element}: the state speaks"));
        for (position, line) in screen.lines.iter().enumerate() {
            checks.wraps_whole(line, &format!("{element}, line {position}"));
        }
    }
}

fn duplicate_rows(checks: &mut Checks, marker: MainThreadMarker) {
    let member = MemberModel {
        path: LONG_PATH.to_owned(),
        size: 2_100_000_000,
        frees: 2_100_000_000,
        clone: false,
        checked: true,
        refusal: None,
        node: None,
    };
    let actions = ShellActions::new(marker);
    let row = member_row(marker, &member, Some(0), false, DUPLICATE_ROW, &palette(), &actions);
    checks.mode(&row.path, NSLineBreakMode::ByTruncatingMiddle, "duplicate path");
    checks.truncated_with_full_text(&row.path, LONG_PATH, "duplicate path");
    let size = row.size.as_ref().expect("a copy shows its size");
    checks.never_cut(size, "duplicate size");
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the truncation check runs on the main thread");
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let mut checks = Checks::default();
    level_rows(&mut checks, marker);
    marked_level_rows(&mut checks, marker);
    sidebar(&mut checks, marker);
    crumbs(&mut checks, marker);
    head(&mut checks, marker);
    scanning(&mut checks, marker);
    hover(&mut checks, marker);
    recommendation_card(&mut checks, marker);
    project_rows(&mut checks, marker);
    duplicate_rows(&mut checks, marker);
    tray_application_group(&mut checks, marker);
    tray_unmark_row(&mut checks, marker);
    purge_sheet(&mut checks, marker);
    if checks.failures.is_empty() {
        println!("truncation: {} checks passed", checks.count);
        ExitCode::SUCCESS
    } else {
        eprintln!("truncation failed:\n{}", checks.failures.join("\n"));
        ExitCode::FAILURE
    }
}
