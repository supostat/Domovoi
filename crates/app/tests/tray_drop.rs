use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_app::{DropKind, DropVerdict, Session, SessionPaths, TraySummary, strings};
use domovoi_index::NodeId;
use domovoi_rules::DenyReason;
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};
use domovoi_theme::format_size;

const CHFLAGS: &str = "/usr/bin/chflags";
const ALLOWED: &str = "Projects";
const LOCKED: &str = "Locked";
const PARENT: &str = "Archive";
const CHILD: &str = "Archive/2019";

struct ScannedVolume {
    _volume: ImageVolume,
    _state: TempTree,
    root: PathBuf,
    session: Session,
}

impl ScannedVolume {
    fn open() -> Self {
        let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
        let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
        populate(&root);
        let state = TempTree::new().expect("a state folder is created");
        let mut session =
            Session::open(&root, &SessionPaths::in_directory(state.path()), "0.0.0-test").expect("the session opens");
        session.scan(None, CancelToken::default()).expect("the volume scans");
        Self {
            _volume: volume,
            _state: state,
            root,
            session,
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn node(&self, relative: &str) -> NodeId {
        let index = self.session.index().expect("the volume is indexed");
        index
            .find(&self.path(relative))
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    }

    fn estimate(&self, relative: &str) -> u64 {
        let index = self.session.index().expect("the volume is indexed");
        index.reclaimable_estimate(self.node(relative)).bytes
    }

    fn marked_paths(&self) -> Vec<PathBuf> {
        self.session
            .tray()
            .items()
            .iter()
            .map(|item| item.path.clone())
            .collect()
    }
}

fn populate(root: &Path) {
    for (folder, bytes) in [
        ("Projects/app", 64 * 1024),
        ("Archive/2019", 32 * 1024),
        ("Locked", 8 * 1024),
    ] {
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

#[test]
fn a_folder_the_denylist_allows_is_offered_with_its_estimate_and_added_without_a_rule() {
    let mut volume = ScannedVolume::open();
    let allowed = vec![volume.path(ALLOWED)];
    let estimate = volume.estimate(ALLOWED);

    let verdict = volume.session.judge_drop(&allowed);

    assert_eq!(verdict.allowed, [volume.node(ALLOWED)]);
    assert_eq!(verdict.allowed_bytes, estimate);
    let hover = verdict.hover();
    assert_eq!(hover.kind, DropKind::Add);
    assert_eq!(
        hover.title,
        format!("Добавить 1\u{a0}объект · ≈ {}", format_size(estimate))
    );
    assert_eq!(verdict.refusal_line(), None);

    let dropped = volume.session.drop_into_tray(&allowed);

    assert_eq!(dropped, verdict, "the drop judges the same way as the hover");
    assert_eq!(volume.marked_paths(), allowed);
    assert_eq!(
        volume.session.tray().items()[0].rule_id,
        None,
        "a dropped row carries no rule"
    );
    let summary = volume.session.tray_summary();
    assert_eq!(
        summary,
        TraySummary {
            objects: 1,
            bytes: estimate
        }
    );
    assert_eq!(summary.title(), format!("К удалению · 1 · ≈ {}", format_size(estimate)));
}

#[test]
fn a_protected_folder_is_refused_with_its_reason_and_a_mixed_drop_adds_the_rest() {
    let mut volume = ScannedVolume::open();
    let locked = vec![volume.path(LOCKED)];

    let verdict = volume.session.judge_drop(&locked);

    assert!(verdict.allowed.is_empty());
    assert!(
        matches!(verdict.refused[0].reason, DenyReason::ProtectedFlag(_)),
        "{:?}",
        verdict.refused
    );
    let hover = verdict.hover();
    assert_eq!(hover.kind, DropKind::Denied);
    assert_eq!(hover.title, "Нельзя: защищено системой");
    assert_eq!(
        verdict.refusal_line().as_deref(),
        Some("Не добавлено: Locked — защищено системой")
    );
    assert_eq!(volume.session.drop_into_tray(&locked), verdict);
    assert!(
        volume.session.tray().items().is_empty(),
        "a refused folder never reaches the tray"
    );

    let mixed = vec![volume.path(ALLOWED), volume.path(LOCKED)];
    let mixed_verdict = volume.session.drop_into_tray(&mixed);

    assert_eq!(
        mixed_verdict.hover().title,
        format!("Добавить 1 из 2 · ≈ {}", format_size(volume.estimate(ALLOWED)))
    );
    assert_eq!(volume.marked_paths(), [volume.path(ALLOWED)]);
}

#[test]
fn a_folder_inside_a_marked_one_is_already_there_and_a_marked_parent_takes_over_its_child() {
    let mut volume = ScannedVolume::open();
    volume.session.drop_into_tray(&[volume.path(CHILD)]);

    let parent = volume.session.drop_into_tray(&[volume.path(PARENT)]);

    assert_eq!(parent.allowed, [volume.node(PARENT)]);
    assert_eq!(
        volume.marked_paths(),
        [volume.path(PARENT)],
        "the parent replaces its marked child"
    );

    let inside = volume.session.judge_drop(&[volume.path(CHILD)]);

    assert_eq!(inside.already, 1);
    assert!(inside.allowed.is_empty() && inside.refused.is_empty());
    let hover = inside.hover();
    assert_eq!(hover.kind, DropKind::Already);
    assert_eq!(hover.title, strings::DROP_ALREADY);
}

#[test]
fn a_path_outside_the_index_is_skipped_and_an_empty_tray_reads_as_the_plain_title() {
    let volume = ScannedVolume::open();
    let missing = vec![volume.path("Gone")];

    assert_eq!(volume.session.judge_drop(&missing), DropVerdict::default());
    let summary = volume.session.tray_summary();
    assert_eq!(summary, TraySummary::default());
    assert_eq!(summary.title(), strings::TRAY);
}
