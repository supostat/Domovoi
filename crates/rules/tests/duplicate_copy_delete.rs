use std::fs::{self, File};
use std::path::Path;

use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, Reclaimable};
use domovoi_rules::{
    CaseRule, ContentComparison, DeleteFailure, DeleteRefusal, DeleteSubject, DenyReason, Denylist, Tray, delete,
};
use domovoi_scanner::{CancelToken, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const KEPT: &str = "Documents/report.pdf";
const COPY: &str = "Downloads/report copy.pdf";
const KEPT_FOLDER: &str = "Documents";
const SIZE: u64 = 3 * 1024 * 1024;

fn volume() -> TempTree {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.root().expect("the root opens");
    for (folder, name) in [("Documents", "report.pdf"), ("Downloads", "report copy.pdf")] {
        root.directory(folder)
            .expect("a folder is created")
            .file(name, SIZE, Fill::Noise(7))
            .expect("a file is written");
    }
    tree
}

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("the root kind is known"),
        &Enrichment::default(),
    )
}

fn node_at(index: &Index, relative: &str) -> NodeId {
    Path::new(relative).iter().fold(index.root(), |node, name| {
        index
            .child_named(node, name)
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    })
}

fn taken(index: &Index, tray: &mut Tray) -> (Vec<DeleteSubject>, Vec<DeleteRefusal>) {
    let sizes: Vec<Reclaimable> = tray
        .items()
        .iter()
        .map(|item| index.reclaimable(item.node).expect("the item is measured"))
        .collect();
    let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
    tray.take_for_delete(index, &denylist, &sizes)
}

fn marked_copy(index: &Index) -> Tray {
    let mut tray = Tray::default();
    tray.add_duplicate_copy(index, node_at(index, COPY), node_at(index, KEPT));
    tray
}

#[test]
fn a_copy_identical_to_its_kept_file_is_deleted() {
    let tree = volume();
    let index = index_of(tree.path());
    let (subjects, refusals) = taken(&index, &mut marked_copy(&index));
    assert!(refusals.is_empty(), "{refusals:?}");
    assert_eq!(
        subjects[0].duplicate_of.as_deref(),
        Some(index.physical_path(node_at(&index, KEPT)).as_path()),
        "the copy carries the file it duplicates"
    );

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(report.deleted, subjects);
    assert!(!tree.path().join(COPY).exists(), "the copy is gone");
    assert!(tree.path().join(KEPT).exists(), "the kept file stays");
}

#[test]
fn a_copy_that_no_longer_matches_its_kept_file_stays() {
    let tree = volume();
    let index = index_of(tree.path());
    let (subjects, _) = taken(&index, &mut marked_copy(&index));
    let edited = vec![1_u8; usize::try_from(SIZE).expect("the size fits")];
    fs::write(tree.path().join(COPY), edited).expect("the copy is edited in place");

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(report.failed, [(subjects[0].clone(), DeleteFailure::DiffersFromKept)]);
    assert!(tree.path().join(COPY).exists(), "an edited copy is not deleted");
}

#[test]
fn a_copy_whose_kept_file_is_marked_as_well_is_refused() {
    let tree = volume();
    let index = index_of(tree.path());
    let mut tray = marked_copy(&index);
    tray.add(&index, node_at(&index, KEPT_FOLDER), None);

    let (subjects, refusals) = taken(&index, &mut tray);

    assert_eq!(
        refusals,
        [DeleteRefusal {
            path: index.path(node_at(&index, COPY)),
            reason: DenyReason::KeptCopyMarked,
        }],
        "the last copy of a file never leaves with its duplicate"
    );
    assert_eq!(
        subjects.iter().map(|subject| subject.path.clone()).collect::<Vec<_>>(),
        [index.path(node_at(&index, KEPT_FOLDER))]
    );
}

#[test]
fn a_copy_whose_kept_file_left_the_index_is_refused() {
    let tree = volume();
    let index = index_of(tree.path());
    let tray = marked_copy(&index);
    fs::remove_file(tree.path().join(KEPT)).expect("the kept file is removed");
    let after = index_of(tree.path());

    let (subjects, refusals) = taken(&after, &mut tray.remapped(&after, |_, path| after.find(path)));

    assert!(subjects.is_empty(), "{subjects:?}");
    assert_eq!(
        refusals,
        [DeleteRefusal {
            path: after.path(node_at(&after, COPY)),
            reason: DenyReason::KeptCopyMissing,
        }]
    );
}

#[test]
fn a_copy_whose_kept_file_vanished_before_the_delete_stays() {
    let tree = volume();
    let index = index_of(tree.path());
    let (subjects, _) = taken(&index, &mut marked_copy(&index));
    fs::remove_file(tree.path().join(KEPT)).expect("the kept file is removed");

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(
        report.failed,
        [(subjects[0].clone(), DeleteFailure::KeptCopyUnavailable)]
    );
    assert!(tree.path().join(COPY).exists(), "the only copy left is not deleted");
}

#[test]
fn a_comparison_stops_when_the_delete_is_stopped() {
    let tree = volume();
    let kept = File::open(tree.path().join(KEPT)).expect("the kept file opens");
    let copy = File::open(tree.path().join(COPY)).expect("the copy opens");
    let cancel = CancelToken::default();
    cancel.cancel();

    let comparison = ContentComparison::of(&copy, &kept, SIZE, &cancel).expect("the files read");

    assert_eq!(comparison, ContentComparison::Stopped);
}
