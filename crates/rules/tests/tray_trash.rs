use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use domovoi_apfs_sys::{current_user_id, link_id};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_rules::{CaseRule, Denylist, TrashFailure, TrashJournal, TrashSubject, Tray};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, ImageVolume, TempTree, Visibility, VolumeFormat};

const LARGE_TREE_FILES: usize = 10_000;
const LARGE_TREE_BUDGET: Duration = Duration::from_secs(1);

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("tree scans");
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

fn journal_in(tree: &TempTree) -> TrashJournal {
    TrashJournal::open(&tree.path().join("journal")).expect("journal opens")
}

#[test]
fn trash_keeps_the_file_identity_and_the_journal_brings_it_back() {
    let tree = TempTree::new().expect("temp tree is created");
    let workspace = TempTree::new().expect("journal workspace is created");
    tree.root()
        .expect("root opens")
        .file("report.pdf", 8_192, Fill::Noise(1))
        .expect("file is written");
    let original = tree.path().join("report.pdf");
    let inode = fs::metadata(&original).expect("metadata").ino();
    let index = index_of(tree.path());
    let mut journal = journal_in(&workspace);
    let mut tray = Tray::default();
    tray.add(&index, child(&index, "report.pdf"), Some("downloads.installers"));
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let outcomes = tray.move_to_trash(&index, &denylist, &mut journal).expect("batch runs");
    let trashed = outcomes[0].result.clone().expect("file goes to the Trash");
    assert!(!original.exists());
    assert_eq!(fs::metadata(&trashed).expect("trashed metadata").ino(), inode);
    assert_eq!(journal.entries().len(), 1);
    assert_eq!(journal.entries()[0].rule_id.as_deref(), Some("downloads.installers"));
    let reopened = journal_in(&workspace);
    assert_eq!(reopened.entries(), journal.entries());
    let restored = journal.restore(0).expect("file comes back");
    assert_eq!(
        fs::canonicalize(&restored).expect("restored path"),
        fs::canonicalize(&original).expect("original")
    );
    assert_eq!(fs::metadata(&original).expect("restored metadata").ino(), inode);
    assert!(journal.entries().is_empty());
}

#[test]
fn ten_thousand_files_move_to_the_trash_within_a_second() {
    let tree = TempTree::new().expect("temp tree is created");
    let workspace = TempTree::new().expect("journal workspace is created");
    let bulk = tree
        .root()
        .expect("root opens")
        .directory("bulk")
        .expect("directory is created");
    for number in 0..LARGE_TREE_FILES {
        bulk.file(format!("file-{number}"), 16, Fill::Byte(1))
            .expect("file is written");
    }
    let index = index_of(tree.path());
    let mut journal = journal_in(&workspace);
    let mut tray = Tray::default();
    tray.add(&index, child(&index, "bulk"), None);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let started = Instant::now();
    let outcomes = tray.move_to_trash(&index, &denylist, &mut journal).expect("batch runs");
    let elapsed = started.elapsed();
    assert!(outcomes[0].result.is_ok(), "{outcomes:?}");
    assert!(elapsed <= LARGE_TREE_BUDGET, "moving took {elapsed:?}");
    journal.restore(0).expect("directory comes back");
    assert_eq!(
        fs::read_dir(tree.path().join("bulk")).expect("bulk lists").count(),
        LARGE_TREE_FILES
    );
}

#[test]
fn a_read_only_volume_fails_per_item_without_stopping_the_batch() {
    let mut volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("image attaches");
    for name in ["warmup", "first", "second"] {
        fs::write(volume.mount_point().join(name), b"data").expect("file is written");
    }
    let workspace = TempTree::new().expect("journal workspace is created");
    let mut journal = journal_in(&workspace);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let writable = index_of(volume.mount_point());
    let mut tray = Tray::default();
    tray.add(&writable, child(&writable, "warmup"), None);
    let warmup = tray
        .move_to_trash(&writable, &denylist, &mut journal)
        .expect("batch runs");
    let trashed = warmup[0].result.clone().expect("a writable image has its own Trash");
    assert!(
        trashed.starts_with(volume.mount_point().join(".Trashes")),
        "{}",
        trashed.display()
    );

    volume.detach().expect("image detaches");
    volume.attach_read_only().expect("image reattaches read-only");
    let mut denylist = denylist;
    denylist.refresh().expect("mounts are listed");
    let read_only = index_of(volume.mount_point());
    tray.add(&read_only, child(&read_only, "first"), None);
    tray.add(&read_only, child(&read_only, "second"), None);
    let outcomes = tray
        .move_to_trash(&read_only, &denylist, &mut journal)
        .expect("batch runs");
    assert_eq!(outcomes.len(), 2);
    for outcome in &outcomes {
        assert_eq!(
            outcome.result,
            Err(TrashFailure::NoTrashOnVolume),
            "{}",
            outcome.path.display()
        );
    }
    assert_eq!(journal.entries().len(), 1);
}

fn temporary_subject(original: &Path, temporary: PathBuf, content: &[u8]) -> TrashSubject {
    fs::write(&temporary, content).expect("the temporary file is written");
    TrashSubject {
        original: original.to_path_buf(),
        original_physical: original.to_path_buf(),
        link_id: link_id(&temporary).expect("the temporary file has a link id"),
        location: temporary,
        alloc_size: 0,
        flags: 0,
        clone_id: 0,
        rule_id: None,
    }
}

#[test]
fn an_item_under_a_temporary_name_lands_in_the_trash_under_its_original_name() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("image attaches");
    let mount_point = fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    let folder = mount_point.join("Media");
    fs::create_dir(&folder).expect("the folder is created");
    let original = folder.join("photo.jpg");
    let workspace = TempTree::new().expect("journal workspace is created");
    let mut journal = journal_in(&workspace);
    let trash = mount_point.join(".Trashes").join(current_user_id().to_string());

    let first = journal
        .trash(&temporary_subject(
            &original,
            folder.join(".domovoi-tmp-first"),
            b"first",
        ))
        .expect("the journal is written")
        .expect("the first item goes to the Trash");
    let second = journal
        .trash(&temporary_subject(
            &original,
            folder.join(".domovoi-tmp-second"),
            b"second",
        ))
        .expect("the journal is written")
        .expect("the second item goes to the Trash");

    assert_eq!(first, trash.join("photo.jpg"));
    assert_eq!(second, trash.join("photo 2.jpg"));
    assert_eq!(fs::read(&first).expect("the first item reads"), b"first");
    assert_eq!(fs::read(&second).expect("the second item reads"), b"second");
    let journaled: Vec<&str> = journal.entries().iter().map(|entry| entry.trashed.as_str()).collect();
    assert_eq!(
        journaled,
        [first.to_str().expect("UTF-8"), second.to_str().expect("UTF-8")]
    );
    let restored = journal.restore(1).expect("the renamed item comes back");
    assert_eq!(restored, original);
    assert_eq!(fs::read(&original).expect("the restored item reads"), b"second");
}
