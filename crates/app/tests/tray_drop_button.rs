use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use domovoi_app::{
    DropKind, LevelList, LevelRow, Session, SessionPaths, ShellActions, ShellPalette, TrayDropButton, TrayDropLook,
    development_fonts, strings,
};
use domovoi_index::Category;
use domovoi_render::register_fonts;
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};
use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSDragOperation, NSPasteboardItem};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString};

const CHFLAGS: &str = "/usr/bin/chflags";
const NODE_TYPE: &str = "com.domovoi.node";
const ALLOWED: &str = "Projects";
const INSIDE: &str = "Projects/app";
const LOCKED: &str = "Locked";

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
}

fn palette() -> ShellPalette {
    ShellPalette {
        tokens: &Palette::DARK,
        high_contrast: false,
    }
}

fn populate(root: &Path) {
    for (folder, bytes) in [("Projects/app", 64 * 1024), ("Locked", 8 * 1024)] {
        let directory = root.join(folder);
        fs::create_dir_all(&directory).expect("a folder is created");
        fs::write(directory.join("payload.bin"), vec![7_u8; bytes]).expect("a file is written");
    }
    let locked = Command::new(CHFLAGS)
        .arg("uchg")
        .arg(root.join(LOCKED))
        .status()
        .expect("chflags runs");
    assert!(locked.success(), "the folder is locked");
}

fn paths(root: &Path, relative: &str) -> Vec<PathBuf> {
    vec![root.join(relative)]
}

fn arm_and_enter(
    checks: &mut Checks,
    button: &TrayDropButton,
    session: &Session,
    dragged: &[PathBuf],
    expected: (DropKind, NSDragOperation, TrayDropLook),
) {
    let (kind, operation, look) = expected;
    let rest = button.shown_title();
    let hover = session.judge_drop(dragged).hover();
    checks.expect(
        hover.kind == kind,
        format!("{dragged:?}: kind {:?}, expected {kind:?}", hover.kind),
    );
    let title = hover.title.clone();
    button.arm(hover);
    checks.expect(button.look() == TrayDropLook::Waiting, "an armed button waits");
    checks.expect(
        button.shown_title() == rest,
        "an armed button keeps its summary until the drag enters",
    );
    let entered = button.show_hover();
    checks.expect(
        entered == operation,
        format!("{title}: operation {entered:?}, expected {operation:?}"),
    );
    checks.expect(
        button.look() == look,
        format!("{title}: look {:?}, expected {look:?}", button.look()),
    );
    checks.expect(
        button.shown_title() == title,
        format!("over the button: {}", button.shown_title()),
    );
    button.leave_hover();
    checks.expect(button.look() == TrayDropLook::Waiting, "leaving the button waits again");
    checks.expect(
        button.shown_title() == rest,
        "leaving the button shows the summary again",
    );
    button.disarm();
    checks.expect(
        button.look() == TrayDropLook::Rest,
        "a finished drag leaves the button at rest",
    );
}

fn button_follows_the_real_verdict(checks: &mut Checks, marker: MainThreadMarker) {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&root);
    let state = TempTree::new().expect("a state folder is created");
    let mut session =
        Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    let actions = ShellActions::new(marker);
    let button = TrayDropButton::new(marker, &palette(), &actions);

    button.show_summary(&session.tray_summary().title());
    checks.expect(
        button.shown_title() == strings::TRAY,
        "an empty tray reads as the plain title",
    );
    let rest_width = button.frame().size.width;

    arm_and_enter(
        checks,
        &button,
        &session,
        &paths(&root, ALLOWED),
        (DropKind::Add, NSDragOperation::Copy, TrayDropLook::Add),
    );
    arm_and_enter(
        checks,
        &button,
        &session,
        &paths(&root, LOCKED),
        (DropKind::Denied, NSDragOperation::None, TrayDropLook::Denied),
    );

    session.drop_into_tray(&paths(&root, ALLOWED));
    let summary = session.tray_summary();
    button.show_summary(&summary.title());
    checks.expect(
        button.shown_title() == summary.title(),
        format!("the summary shows on the button: {}", button.shown_title()),
    );
    checks.expect(
        button.frame().size.width > rest_width,
        "the button widens for the collected count and size",
    );

    arm_and_enter(
        checks,
        &button,
        &session,
        &paths(&root, INSIDE),
        (DropKind::Already, NSDragOperation::None, TrayDropLook::Already),
    );
    checks.expect(
        button.show_hover() == NSDragOperation::None && button.look() == TrayDropLook::Rest,
        "a button that was never armed refuses a drop",
    );
}

fn pasteboard_carries_only_indexed_rows(checks: &mut Checks, marker: MainThreadMarker) {
    let actions = ShellActions::new(marker);
    let list = LevelList::build(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(400.0, 300.0)),
        palette(),
        &actions,
    );
    let row = |node: usize, draggable: bool| LevelRow {
        node,
        name: format!("row {node}"),
        size_text: "1,0 ГБ".to_owned(),
        share: 1.0,
        category: Category::Dev,
        draggable,
        marked: false,
    };
    list.show(vec![row(7, true), row(8, false)]);
    let node_type = NSString::from_str(NODE_TYPE);
    let indexed: Option<Retained<NSPasteboardItem>> =
        unsafe { msg_send![&*list.source, tableView: &*list.table, pasteboardWriterForRow: 0 as NSInteger] };
    let text = indexed
        .as_ref()
        .and_then(|item| item.stringForType(&node_type))
        .map(|text| text.to_string());
    checks.expect(
        text.as_deref() == Some("7"),
        format!("an indexed row carries its node on the pasteboard: {text:?}"),
    );
    let synthetic: Option<Retained<NSPasteboardItem>> =
        unsafe { msg_send![&*list.source, tableView: &*list.table, pasteboardWriterForRow: 1 as NSInteger] };
    checks.expect(synthetic.is_none(), "a synthetic row is not dragged");
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the drop button check runs on the main thread");
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts).expect("the fonts register");
    }
    let mut checks = Checks::default();
    button_follows_the_real_verdict(&mut checks, marker);
    pasteboard_carries_only_indexed_rows(&mut checks, marker);
    if checks.failures.is_empty() {
        println!("tray drop button: {} checks passed", checks.count);
        ExitCode::SUCCESS
    } else {
        eprintln!("tray drop button failed:\n{}", checks.failures.join("\n"));
        ExitCode::FAILURE
    }
}
