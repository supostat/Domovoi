use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use domovoi_apfs_sys::link_id;
use domovoi_app::PurgeJob;
use domovoi_rules::{TrashJournal, TrashSubject};
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat, test_timeout};

const FOLDER_FILES: usize = 200;
const FILE_BYTES: usize = 4_096;
const POLL: Duration = Duration::from_millis(5);

fn trash(journal: &mut TrashJournal, location: &Path) {
    let subject = TrashSubject {
        original: location.to_path_buf(),
        original_physical: location.to_path_buf(),
        location: location.to_path_buf(),
        link_id: link_id(location).expect("the item has a link id"),
        alloc_size: 0,
        flags: 0,
        clone_id: 0,
        rule_id: None,
    };
    journal
        .trash(&subject)
        .expect("the batch runs")
        .expect("the item goes to the Trash");
}

#[test]
fn a_purge_job_removes_the_moved_set_and_reports_its_progress() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    let folder = root.join("node_modules");
    fs::create_dir(&folder).expect("the folder is created");
    for number in 0..FOLDER_FILES {
        fs::write(folder.join(format!("module-{number}.js")), vec![3; FILE_BYTES]).expect("the file is written");
    }
    let file = root.join("archive.zip");
    fs::write(&file, vec![4; FILE_BYTES]).expect("the file is written");
    let workspace = TempTree::new().expect("the journal workspace is created");
    let mut journal = TrashJournal::open(&workspace.path().join("journal")).expect("the journal opens");
    trash(&mut journal, &folder);
    trash(&mut journal, &file);
    let entries = journal.take_for_purge().expect("the journal gives its entries");

    let job = PurgeJob::start(entries.clone());
    let deadline = Instant::now() + test_timeout();
    let mut seen = Vec::new();
    while !job.is_finished() {
        assert!(Instant::now() < deadline, "the purge did not finish in time");
        seen.push(job.progress().done);
        thread::sleep(POLL);
    }
    let last = job.progress();
    let report = job.finish();

    assert!(
        seen.windows(2).all(|pair| pair[0] <= pair[1]),
        "progress only grows: {seen:?}"
    );
    assert_eq!(last.done, last.total, "progress reaches the total");
    assert_eq!(last.total, FOLDER_FILES + 2);
    assert_eq!(report.purged, entries);
    assert!(
        entries
            .iter()
            .all(|entry| Path::new(&entry.trashed).symlink_metadata().is_err()),
        "the Trash no longer holds the moved set"
    );
}
