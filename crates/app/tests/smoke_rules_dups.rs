use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use domovoi_app::{
    CardModel, DeleteOutcome, DuplicatesModel, ProjectRow, Provenance, RecommendationsModel, SafetyBadge, Session,
    SessionPaths, TrayMeasureJob, TrayModel, strings,
};
use domovoi_index::Reclaimable;
use domovoi_rules::delete;
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};

const MEGABYTE: usize = 1024 * 1024;
const BUILD_OUTPUT: usize = 2 * MEGABYTE;
const DUPLICATE: usize = 3 * MEGABYTE / 2;
const NPM_CACHE: &str = "Users/tester/.npm/_cacache/content-v2";
const NPM_CACHE_RULE: &str = "cache.npm";

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
    let npm_cache = root.join(NPM_CACHE);
    fs::create_dir_all(&npm_cache).expect("the npm cache is created");
    fs::write(npm_cache.join("package.tgz"), noise(MEGABYTE, 13)).expect("a cached package is written");
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

fn the_demo_project(recommendations: &RecommendationsModel) -> (&CardModel, &ProjectRow) {
    let card = recommendations
        .cards
        .iter()
        .find(|card| card.provenance == Provenance::Projects)
        .expect("the Rust build output is recommended as a project");
    assert_eq!(card.title, strings::PROJECTS_TITLE);
    assert_eq!(card.level, SafetyBadge::Safe);
    assert!(card.amount >= BUILD_OUTPUT as u64, "{} bytes", card.amount);
    assert!(card.paths[0].ends_with("Projects/demo/target"), "{:?}", card.paths);
    let [demo] = card.rows.as_slice() else {
        panic!("the demo project has one row: {:?}", card.rows);
    };
    assert_eq!(demo.name, "demo");
    assert!(demo.path.ends_with("Projects/demo"), "{}", demo.path);
    assert_eq!(demo.artifacts, ["target"]);
    assert!(
        !demo.stale && !demo.selected,
        "a project changed today is fresh and nothing is checked before the user checks it"
    );
    assert!(
        demo.targets
            .iter()
            .all(|target| target.rule_id.as_deref() == Some("cargo.target")
                && target
                    .project
                    .as_ref()
                    .is_some_and(|root| root.ends_with("Projects/demo"))),
        "{:?}",
        demo.targets
    );
    (card, demo)
}

fn estimated_then_measured_tray(session: &Session, title: &str) -> (TrayModel, TrayModel, Vec<Reclaimable>) {
    let index = session.index().expect("the index stays");
    let estimated = TrayModel::estimated(index, session.tray());
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
    let measured = TrayModel::measured(index, session.tray(), &exact);
    assert!(measured.measuring.is_none());
    assert_eq!(measured.rows[0].title, title);
    (estimated, measured, exact)
}

fn searched_with_the_own_copy_checked(session: &mut Session) -> DuplicatesModel {
    session
        .find_duplicates(None, CancelToken::default())
        .expect("duplicates are searched");
    let index = session.index().expect("the index stays");
    let mut duplicates =
        DuplicatesModel::found(index, session.duplicates().expect("the search stores duplicates"), 3, 1);
    assert_eq!(duplicates.groups().len(), 1);
    let group = &duplicates.groups()[0];
    assert_eq!(group.copies.len(), 2);
    assert_eq!(
        group.copies.iter().filter(|copy| copy.clone).count(),
        1,
        "the APFS clone is recognised"
    );
    assert!(
        duplicates.checked_copies().is_empty() && duplicates.will_free() == 0,
        "no copy is checked before the user checks it"
    );
    assert!(duplicates.can_free() >= DUPLICATE as u64);
    duplicates.groups_mut()[0]
        .copies
        .iter_mut()
        .find(|member| !member.clone)
        .expect("one copy keeps its own blocks")
        .checked = true;
    assert_eq!(duplicates.will_free(), duplicates.can_free());
    duplicates
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
    let (card, demo) = the_demo_project(&recommendations);
    let npm_cache = recommendations
        .cards
        .iter()
        .find(|card| card.provenance == Provenance::Rule(NPM_CACHE_RULE.to_owned()))
        .expect("the npm cache makes a rule card");
    assert!(
        npm_cache.level == SafetyBadge::Safe && npm_cache.selectable() && !npm_cache.selected,
        "a safe card that can be checked is not checked before the user checks it"
    );
    assert_eq!(
        recommendations.selected_total(),
        0,
        "nothing is checked before the user checks it"
    );

    let mut duplicates = searched_with_the_own_copy_checked(&mut session);
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
        session.add_target_to_tray(target);
    }
    let (estimated, measured, sizes) = estimated_then_measured_tray(&session, &demo.name);
    let expected = measured.total();
    let (subjects, refusals) = session
        .take_tray_for_delete(&sizes)
        .expect("running programs are checked");
    assert!(refusals.is_empty(), "{refusals:?}");
    assert!(session.tray().items().is_empty(), "taking empties the tray");
    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});
    assert_eq!(report.deleted, subjects);
    assert_eq!(report.freed_bytes, expected);
    session
        .settle_delete(&report)
        .expect("the index forgets the deleted paths");
    assert!(!mount_point.join("Projects/demo/target").exists());
    assert!(
        session
            .index()
            .expect("the index stays")
            .find(&mount_point.join("Projects/demo/target"))
            .is_none(),
        "the deleted folder leaves the index"
    );
    let outcome = DeleteOutcome::from_report(&report, &refusals);
    assert_eq!(outcome.objects, subjects.len());
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    let after = measured.with_deleted(outcome);
    assert_eq!(after.deleted.as_ref().map(|outcome| outcome.freed), Some(expected));
    assert!(estimated.measuring.is_some() && after.measuring.is_none());

    session
        .replace_with_clone(keep, copy)
        .expect("the copy becomes a clone");
    let index = session.index().expect("the index stays");
    let replaced = DuplicatesModel::found(index, session.duplicates().expect("duplicates stay"), 3, 1);
    assert!(
        replaced.groups()[0].copies.iter().all(|member| member.clone),
        "every copy now shares blocks with the kept file"
    );
    assert_eq!(replaced.can_free(), 0, "nothing is left to free");
}
