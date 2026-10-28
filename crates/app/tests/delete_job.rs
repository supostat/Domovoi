use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_app::DeleteJob;
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, Reclaimable};
use domovoi_rules::{CaseRule, Denylist, Tray};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{ImageVolume, Visibility, VolumeFormat, test_timeout};

const FOLDER_FILES: usize = 200;
const FILE_BYTES: usize = 4_096;
const POLL: Duration = Duration::from_millis(5);

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the volume scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn child(index: &Index, name: &str) -> NodeId {
    index
        .child_named(index.root(), name.as_ref())
        .unwrap_or_else(|| panic!("{name} is indexed"))
}

#[test]
fn a_delete_job_removes_the_taken_tray_and_reports_its_progress() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    let folder = root.join("node_modules");
    fs::create_dir(&folder).expect("the folder is created");
    for number in 0..FOLDER_FILES {
        fs::write(folder.join(format!("module-{number}.js")), vec![3; FILE_BYTES]).expect("the file is written");
    }
    let file = root.join("archive.zip");
    fs::write(&file, vec![4; FILE_BYTES]).expect("the file is written");
    let index = index_of(&root);
    let mut tray = Tray::default();
    tray.add(&index, child(&index, "node_modules"), Some("dev.deps"));
    tray.add(&index, child(&index, "archive.zip"), None);
    let sizes: Vec<Reclaimable> = tray
        .items()
        .iter()
        .map(|item| index.reclaimable(item.node).expect("the item is measured"))
        .collect();
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let (subjects, refusals) = tray.take_for_delete(&index, &denylist, &sizes);
    assert!(refusals.is_empty(), "{refusals:?}");
    let taken: Vec<PathBuf> = subjects.iter().map(|subject| subject.physical.clone()).collect();

    let job = DeleteJob::start(subjects.clone());
    let deadline = Instant::now() + test_timeout();
    let mut seen = Vec::new();
    while !job.is_finished() {
        assert!(Instant::now() < deadline, "the delete did not finish in time");
        seen.push(job.progress().files_done);
        thread::sleep(POLL);
    }
    let last = job.progress();
    let report = job.finish();

    assert!(
        seen.windows(2).all(|pair| pair[0] <= pair[1]),
        "progress only grows: {seen:?}"
    );
    assert_eq!(last.files_done, last.files_total, "progress reaches the total");
    assert_eq!(
        last.files_total,
        FOLDER_FILES + 1,
        "the files of the folder and the archive, counted by the index"
    );
    assert_eq!(report.deleted, subjects);
    assert_eq!(
        report.freed_bytes,
        sizes.iter().map(|size| size.bytes).sum::<u64>(),
        "the freed bytes are the measured sizes of the taken items"
    );
    assert!(
        taken.iter().all(|path| path.symlink_metadata().is_err()),
        "the disk no longer holds the taken items"
    );
}
