use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use domovoi_apfs_sys::link_id;
use domovoi_rules::{JournalEntry, PurgeProgress, PurgeSkip, TrashJournal, TrashSubject, purge};
use domovoi_scanner::CancelToken;
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};

const FOLDER_FILES: usize = 300;
const FILE_BYTES: usize = 4_096;

struct Trashed {
    _volume: ImageVolume,
    _workspace: TempTree,
    journal_path: PathBuf,
    journal: TrashJournal,
}

fn subject(location: &Path) -> TrashSubject {
    TrashSubject {
        original: location.to_path_buf(),
        original_physical: location.to_path_buf(),
        location: location.to_path_buf(),
        link_id: link_id(location).expect("the item has a link id"),
        alloc_size: 0,
        flags: 0,
        clone_id: 0,
        rule_id: None,
    }
}

fn folder(root: &Path, name: &str) -> PathBuf {
    let folder = root.join(name);
    fs::create_dir(&folder).expect("the folder is created");
    for number in 0..FOLDER_FILES {
        fs::write(folder.join(format!("file-{number}.js")), vec![1; FILE_BYTES]).expect("the file is written");
    }
    folder
}

fn file(root: &Path, name: &str) -> PathBuf {
    let file = root.join(name);
    fs::write(&file, vec![2; FILE_BYTES]).expect("the file is written");
    file
}

fn trashed(make: impl FnOnce(&Path) -> Vec<PathBuf>) -> Trashed {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let root = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    let workspace = TempTree::new().expect("the journal workspace is created");
    let journal_path = workspace.path().join("journal");
    let mut journal = TrashJournal::open(&journal_path).expect("the journal opens");
    for item in make(&root) {
        journal
            .trash(&subject(&item))
            .expect("the batch runs")
            .expect("the item goes to the Trash");
    }
    Trashed {
        _volume: volume,
        _workspace: workspace,
        journal_path,
        journal,
    }
}

fn exists(entry: &JournalEntry) -> bool {
    Path::new(&entry.trashed).symlink_metadata().is_ok()
}

#[test]
fn a_purge_removes_what_was_moved_and_empties_the_journal() {
    let mut trashed = trashed(|root| vec![folder(root, "cache"), file(root, "installer.dmg")]);
    let entries = trashed.journal.take_for_purge().expect("the journal gives its entries");
    assert!(
        TrashJournal::open(&trashed.journal_path)
            .expect("the journal reopens")
            .entries()
            .is_empty(),
        "the journal forgets the set before anything is removed"
    );
    let steps = Mutex::new(Vec::new());

    let report = purge(entries.clone(), &CancelToken::default(), &|step: PurgeProgress| {
        steps.lock().expect("the steps are not poisoned").push(step);
    });

    assert_eq!(report.purged, entries);
    assert!(report.untouched.is_empty() && report.skipped.is_empty() && report.failed.is_empty());
    assert!(
        entries.iter().all(|entry| !exists(entry)),
        "the Trash no longer holds them"
    );
    let steps = steps.into_inner().expect("the steps are not poisoned");
    let last = steps
        .iter()
        .max_by_key(|step| step.done)
        .expect("the purge reports progress");
    assert_eq!(last.done, last.total, "progress reaches the total");
    assert_eq!(
        last.total,
        FOLDER_FILES + 2,
        "the folder's children, the folder and the file"
    );
}

#[test]
fn a_purge_leaves_a_replaced_item_and_skips_what_is_gone() {
    let mut trashed = trashed(|root| vec![file(root, "replaced.bin"), file(root, "emptied.bin")]);
    let entries = trashed.journal.take_for_purge().expect("the journal gives its entries");
    let replaced = PathBuf::from(&entries[0].trashed);
    fs::remove_file(&replaced).expect("the trashed file is removed");
    fs::write(&replaced, b"someone else's file").expect("another file takes its place");
    fs::remove_file(&entries[1].trashed).expect("the Trash was emptied elsewhere");

    let report = purge(entries.clone(), &CancelToken::default(), &|_| {});

    assert!(report.purged.is_empty());
    assert_eq!(
        report.skipped,
        [
            (entries[0].clone(), PurgeSkip::Replaced),
            (entries[1].clone(), PurgeSkip::NotInTrash),
        ]
    );
    assert_eq!(
        fs::read(&replaced).expect("the replacement stays"),
        b"someone else's file"
    );
}

#[test]
fn a_cancelled_purge_returns_untouched_entries_to_the_journal() {
    let mut trashed = trashed(|root| vec![folder(root, "first"), folder(root, "second")]);
    let entries = trashed.journal.take_for_purge().expect("the journal gives its entries");
    let cancel = CancelToken::default();

    let report = purge(entries.clone(), &cancel, &|step: PurgeProgress| {
        if step.entries_done == 1 {
            cancel.cancel();
        }
    });

    assert_eq!(report.purged, [entries[0].clone()]);
    assert_eq!(report.untouched, [entries[1].clone()]);
    assert!(!exists(&entries[0]));
    let second = Path::new(&entries[1].trashed);
    assert_eq!(
        fs::read_dir(second).expect("the untouched folder stays").count(),
        FOLDER_FILES,
        "nothing of the untouched entry is removed"
    );
    trashed
        .journal
        .return_untouched(report.untouched)
        .expect("the journal takes the untouched back");
    assert_eq!(
        TrashJournal::open(&trashed.journal_path)
            .expect("the journal reopens")
            .entries(),
        [entries[1].clone()],
        "the untouched entry can be restored again"
    );
}
