use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use domovoi_app::{
    CardModel, DeleteJob, DeleteOutcome, Provenance, RecommendationsModel, Session, SessionPaths, TrayMeasureJob,
    TrayModel,
};
use domovoi_rules::{DeleteFailure, DeleteRefusal, DeleteReport, DeleteSubject, DenyReason};
use domovoi_scanner::CancelToken;
use domovoi_testkit::TempTree;

const APP_VERSION: &str = "0.0.0-test";
const FILE_BYTES: usize = 64 * 1024;
const SECTOR: u64 = 512;
const UNIQUE: &str = "node_modules/pkg/unique.bin";
const SHARED: &str = "node_modules/pkg/shared.bin";
const POLL: Duration = Duration::from_millis(20);

struct Workspace {
    _volume: TempTree,
    _state: TempTree,
    root: PathBuf,
    session: Session,
}

impl Workspace {
    fn new(projects_with_own_files: &[&str], project_with_only_the_shared_file: Option<&str>) -> Self {
        let volume = TempTree::new().expect("a temp volume is created");
        let state = TempTree::new().expect("a temp state is created");
        let root = fs::canonicalize(volume.path()).expect("the volume path resolves");
        for (position, project) in projects_with_own_files.iter().enumerate() {
            let folder = root.join(project);
            write(&folder.join(UNIQUE), u8::try_from(position + 1).expect("a small byte"));
            fs::write(folder.join("package.json"), b"{}").expect("the manifest is written");
        }
        let shared = root.join(projects_with_own_files[0]).join(SHARED);
        write(&shared, 9);
        let partners = projects_with_own_files[1..]
            .iter()
            .chain(project_with_only_the_shared_file.iter());
        for partner in partners {
            let folder = root.join(partner);
            fs::create_dir_all(folder.join("node_modules/pkg")).expect("the dependencies folder is created");
            fs::write(folder.join("package.json"), b"{}").expect("the manifest is written");
            fs::hard_link(&shared, folder.join(SHARED)).expect("the projects share a hard link");
        }
        let paths = SessionPaths::in_directory(state.path()).with_home(&root);
        let mut session = Session::open(&root, &paths, APP_VERSION).expect("the session opens");
        session.scan(None, CancelToken::default()).expect("the volume scans");
        Self {
            _volume: volume,
            _state: state,
            root,
            session,
        }
    }

    fn allocated(&self, relative: &str) -> u64 {
        fs::metadata(self.root.join(relative))
            .expect("the file has metadata")
            .blocks()
            * SECTOR
    }

    fn model(&self) -> RecommendationsModel {
        RecommendationsModel::build(
            self.session.index().expect("the volume is scanned"),
            self.session.rules(),
            self.session
                .recommendations()
                .expect("a scanned volume has recommendations"),
            SystemTime::now(),
        )
    }

    fn put_in_tray(&mut self, card: &CardModel) {
        for target in card.selected_targets() {
            self.session.add_target_to_tray(&target);
        }
    }

    fn tray_items(&self) -> Vec<PathBuf> {
        self.session
            .tray()
            .items()
            .iter()
            .map(|item| item.physical.clone())
            .collect()
    }
}

fn write(path: &Path, byte: u8) {
    fs::create_dir_all(path.parent().expect("a file sits in a folder")).expect("the folder is created");
    fs::write(path, vec![byte; FILE_BYTES]).expect("the file is written");
}

fn projects(model: &mut RecommendationsModel) -> &mut CardModel {
    model
        .cards
        .iter_mut()
        .find(|card| card.provenance == Provenance::Projects)
        .expect("the volume has a projects card")
}

fn row_of(card: &CardModel, name: &str) -> usize {
    card.rows
        .iter()
        .position(|row| row.name == name)
        .unwrap_or_else(|| panic!("{name} has a row"))
}

fn check_only(card: &mut CardModel, names: &[&str]) {
    for row in &mut card.rows {
        row.selected = false;
    }
    for name in names {
        let position = row_of(card, name);
        card.toggle_row(position);
    }
}

#[test]
fn projects_checked_together_count_the_file_they_share_through_a_hard_link() {
    let mut workspace = Workspace::new(&["alpha", "beta"], None);
    let freed_together = workspace.allocated(&format!("alpha/{UNIQUE}"))
        + workspace.allocated(&format!("beta/{UNIQUE}"))
        + workspace.allocated(&format!("alpha/{SHARED}"));
    let mut model = workspace.model();
    let card = projects(&mut model);
    check_only(card, &["alpha", "beta"]);
    assert_eq!(
        card.rows.iter().filter(|row| row.selected).count(),
        2,
        "both projects are checked"
    );
    assert_eq!(
        model.selected_total(),
        freed_together,
        "deleting both projects frees their own files and the file they share"
    );

    let card = projects(&mut model).clone();
    workspace.put_in_tray(&card);
    let index = workspace.session.index().expect("the volume is scanned");
    let estimated = TrayModel::estimated(index, workspace.session.tray());
    assert_eq!(
        estimated.total(),
        freed_together,
        "the sheet estimates the projects together"
    );
    assert_eq!(estimated.held_by_hard_links, 0, "nothing stays held once both go");

    let measured = TrayMeasureJob::start(workspace.tray_items())
        .finish()
        .expect("the tray is measured");
    let shared = workspace.allocated(&format!("alpha/{SHARED}"));
    assert!(
        measured
            .sizes
            .items
            .iter()
            .all(|size| size.held_by_hard_links == shared),
        "each project alone keeps the shared file: {:?}",
        measured.sizes.items
    );
    assert_eq!(measured.sizes.together.bytes, freed_together);
    assert_eq!(measured.sizes.together.held_by_hard_links, 0);
    let sheet = TrayModel::measured(index, workspace.session.tray(), &measured.sizes);
    assert_eq!(
        sheet.total(),
        freed_together,
        "the measured sheet counts the shared file once"
    );
}

#[test]
fn a_project_made_only_of_the_shared_file_is_checked_together_with_its_partner() {
    let workspace = Workspace::new(&["alpha"], Some("gamma"));
    let unique = workspace.allocated(&format!("alpha/{UNIQUE}"));
    let shared = workspace.allocated(&format!("alpha/{SHARED}"));
    let mut model = workspace.model();
    let card = projects(&mut model);
    let gamma = &card.rows[row_of(card, "gamma")];
    assert_eq!(gamma.amount, 0, "the shared file frees nothing while its partner stays");
    assert!(gamma.in_joint_hold);
    assert!(gamma.selectable(), "a project in a joint hold can be checked");

    check_only(card, &["gamma"]);
    assert_eq!(model.selected_total(), 0, "alone the project frees nothing");
    check_only(projects(&mut model), &["alpha", "gamma"]);
    assert_eq!(model.selected_total(), unique + shared);
}

#[test]
fn deleting_both_projects_reports_the_shared_file_as_freed() {
    let mut workspace = Workspace::new(&["alpha", "beta"], None);
    let freed_together = workspace.allocated(&format!("alpha/{UNIQUE}"))
        + workspace.allocated(&format!("beta/{UNIQUE}"))
        + workspace.allocated(&format!("alpha/{SHARED}"));
    let mut model = workspace.model();
    let card = projects(&mut model);
    check_only(card, &["alpha", "beta"]);
    let card = card.clone();
    workspace.put_in_tray(&card);
    let sizes = TrayMeasureJob::start(workspace.tray_items())
        .finish()
        .expect("the tray is measured")
        .sizes;
    let (subjects, refusals) = workspace
        .session
        .take_tray_for_delete(&sizes.items)
        .expect("running programs are checked");
    let job = DeleteJob::start(subjects);
    while !job.is_finished() {
        thread::sleep(POLL);
    }
    let outcome = DeleteOutcome::from_report(&job.finish(), &refusals, sizes.freed_together());
    assert!(outcome.failures.is_empty(), "{outcome:?}");
    assert_eq!(outcome.freed, freed_together, "the outcome matches the sheet");
    assert!(!workspace.root.join("alpha/node_modules").exists());
    assert!(!workspace.root.join("beta/node_modules").exists());
}

#[test]
fn an_item_left_behind_keeps_what_the_items_free_only_together_out_of_the_outcome() {
    let subject = |name: &str, bytes: u64| DeleteSubject {
        path: PathBuf::from(name),
        physical: PathBuf::from(name),
        link_id: 1,
        measured_bytes: bytes,
        indexed_files: 1,
        rule_id: None,
        duplicate_of: None,
    };
    let together = 5;
    let everything = DeleteReport {
        deleted: vec![subject("alpha", 10), subject("beta", 20)],
        freed_bytes: 30,
        ..DeleteReport::default()
    };
    assert_eq!(DeleteOutcome::from_report(&everything, &[], together).freed, 35);
    let failed = DeleteReport {
        deleted: vec![subject("alpha", 10)],
        failed: vec![(subject("beta", 20), DeleteFailure::NoPermission)],
        freed_bytes: 10,
        ..DeleteReport::default()
    };
    assert_eq!(DeleteOutcome::from_report(&failed, &[], together).freed, 10);
    let refused = [DeleteRefusal {
        path: PathBuf::from("gamma"),
        reason: DenyReason::OwnerRunning,
    }];
    assert_eq!(DeleteOutcome::from_report(&everything, &refused, together).freed, 30);
}
