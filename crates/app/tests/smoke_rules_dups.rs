use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use domovoi_app::{
    DuplicatesModel, Provenance, RecommendationsModel, SafetyBadge, Session, SessionPaths, TrayMeasureJob, TrayModel,
    strings,
};
use domovoi_index::Reclaimable;
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};

const MEGABYTE: usize = 1024 * 1024;
const BUILD_OUTPUT: usize = 2 * MEGABYTE;
const DUPLICATE: usize = 3 * MEGABYTE / 2;

fn noise(size: usize, seed: u8) -> Vec<u8> {
    (0..size)
        .map(|position| (position as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn populate(root: &Path) {
    let project = root.join("Projects/demo");
    fs::create_dir_all(project.join("target/debug")).expect("the project tree is created");
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("the manifest is written");
    fs::write(project.join("target/debug/demo"), noise(BUILD_OUTPUT, 7)).expect("the build output is written");
    let media = root.join("Media");
    fs::create_dir_all(&media).expect("the media folder is created");
    let content = noise(DUPLICATE, 11);
    fs::write(media.join("a.bin"), &content).expect("a duplicate is written");
    fs::write(media.join("b.bin"), &content).expect("a duplicate is written");
    let cloned = Command::new("/bin/cp")
        .arg("-c")
        .arg(media.join("a.bin"))
        .arg(media.join("c.bin"))
        .status()
        .expect("cp starts");
    assert!(cloned.success(), "the clone is made");
}

fn estimated_then_measured_tray(session: &Session, title: &str) -> (TrayModel, TrayModel) {
    let index = session.index().expect("the index stays");
    let estimated = TrayModel::estimated(index, session.tray(), session.trash_journal());
    assert_eq!(estimated.rows.len(), 1);
    assert_eq!(estimated.rows[0].title, title);
    assert!(estimated.total() >= BUILD_OUTPUT as u64);
    assert!(estimated.measuring.is_some(), "the sheet opens on the estimate");
    let items: Vec<PathBuf> = session
        .tray()
        .items()
        .iter()
        .map(|item| item.physical.clone())
        .collect();
    let exact = TrayMeasureJob::start(items)
        .finish()
        .expect("the tray is measured")
        .sizes;
    let indexed: Vec<Reclaimable> = session
        .tray()
        .items()
        .iter()
        .map(|item| index.reclaimable(item.node).expect("the index measures the item"))
        .collect();
    assert_eq!(exact, indexed);
    let measured = TrayModel::measured(index, session.tray(), session.trash_journal(), &exact);
    assert!(measured.measuring.is_none());
    assert_eq!(measured.rows[0].title, title);
    (estimated, measured)
}

#[test]
fn session_turns_real_rules_and_duplicates_into_screen_models() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let mount_point = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    populate(&mount_point);
    let state = TempTree::new().expect("a temp tree is created");
    let mut session = Session::open(&mount_point, &SessionPaths::in_directory(state.path()), "0.0.0-test")
        .expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");

    let index = session.index().expect("the scan installs an index");
    let recommendations = RecommendationsModel::build(
        index,
        session.rules(),
        session.recommendations().expect("a scan builds recommendations"),
        SystemTime::now(),
    );
    let card = recommendations
        .cards
        .iter()
        .find(|card| card.provenance == Provenance::Rule("cargo.target".to_owned()))
        .expect("the Rust build output is recommended");
    assert_eq!(
        card.title,
        strings::rule_title("cargo.target").expect("the rule has a title")
    );
    assert_eq!(card.level, SafetyBadge::Safe);
    assert!(card.selected, "safe cards are preselected");
    assert!(card.amount >= BUILD_OUTPUT as u64, "{} bytes", card.amount);
    assert!(card.paths[0].ends_with("Projects/demo/target"), "{:?}", card.paths);
    assert_eq!(recommendations.selected_total(), recommendations.safe);

    session
        .find_duplicates(None, CancelToken::default())
        .expect("duplicates are searched");
    let index = session.index().expect("the index stays");
    let mut duplicates =
        DuplicatesModel::found(index, session.duplicates().expect("the search stores duplicates"), 3, 1)
            .expect("the duplicates model builds");
    assert_eq!(duplicates.groups().len(), 1);
    let group = &duplicates.groups()[0];
    assert_eq!(group.copies.len(), 2);
    assert_eq!(
        group.copies.iter().filter(|copy| copy.clone).count(),
        1,
        "the APFS clone is recognised"
    );
    assert!(duplicates.will_free() >= DUPLICATE as u64);
    duplicates.assess_clones(|keep, copy| session.clone_plan(keep, copy).map(drop));
    let (keep, copy) = duplicates.checked_copies()[0];
    let eligible = duplicates.groups()[0]
        .copies
        .iter()
        .find(|member| member.node == Some(copy))
        .expect("the checked copy is listed");
    assert_eq!(eligible.refusal, None, "a lone, owned, closed copy can become a clone");

    let targets = card.targets.clone();
    for target in &targets {
        session.add_to_tray(target.node, target.rule_id.as_deref());
    }
    let (estimated, measured) = estimated_then_measured_tray(&session, &card.title);
    let moved = measured.total();
    let outcomes = session.move_tray_to_trash().expect("the tray moves to the Trash");
    assert!(outcomes.iter().all(|outcome| outcome.result.is_ok()), "{outcomes:?}");
    assert!(!mount_point.join("Projects/demo/target").exists());
    let unmeasured = estimated.with_outcome(&outcomes, session.trash_journal());
    assert_eq!(unmeasured.outcome.as_ref().map(|outcome| outcome.estimated), Some(true));
    let after = measured.with_outcome(&outcomes, session.trash_journal());
    assert_eq!(after.outcome.as_ref().map(|outcome| outcome.moved), Some(moved));
    assert_eq!(after.outcome.as_ref().map(|outcome| outcome.estimated), Some(false));
    assert_eq!(after.journal.len(), 1);
    assert_eq!(after.journal[0].title, card.title);
    session
        .restore(after.journal[0].position)
        .expect("the build output comes back");
    assert!(mount_point.join("Projects/demo/target/debug/demo").exists());

    session
        .replace_with_clone(keep, copy)
        .expect("the copy becomes a clone");
    let index = session.index().expect("the index stays");
    let replaced = DuplicatesModel::found(index, session.duplicates().expect("duplicates stay"), 3, 1)
        .expect("the duplicates model builds");
    assert!(
        replaced.groups()[0].copies.iter().all(|member| member.clone),
        "every copy now shares blocks with the kept file"
    );
    assert_eq!(replaced.will_free(), 0);
}
