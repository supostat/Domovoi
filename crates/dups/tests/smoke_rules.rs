use std::path::{Path, PathBuf};

use domovoi_dups::{CloudFolders, DuplicateAction, DuplicateOptions, HashingPool, find_duplicates};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_rules::{CaseRule, Denylist, delete};
use domovoi_scanner::{CancelToken, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const MEBIBYTE: u64 = 1024 * 1024;

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind is known"),
        &Enrichment::default(),
    )
}

#[test]
fn duplicate_copies_are_deleted_by_default() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.directory("Documents")
        .expect("directory is created")
        .file("report.pdf", 2 * MEBIBYTE, Fill::Noise(11))
        .expect("file is written");
    let downloads = root.directory("Downloads").expect("directory is created");
    for name in ["report.pdf", "report (1).pdf"] {
        downloads
            .file(name, 2 * MEBIBYTE, Fill::Noise(11))
            .expect("file is written");
    }
    let index = index_of(tree.path());
    let pool = HashingPool::new(None).expect("hashing pool starts");
    let duplicates = find_duplicates(&index, &pool, &DuplicateOptions::new(CloudFolders::new(Vec::new())))
        .expect("search completes");
    assert_eq!(duplicates.groups.len(), 1);
    let group = &duplicates.groups[0];
    assert_eq!(group.copies.len(), 2);
    assert!(group.copies.iter().all(|copy| !copy.shares_with_kept));
    assert_eq!(DuplicateAction::default(), DuplicateAction::Delete);

    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let selected: Vec<NodeId> = group.copies.iter().map(|copy| copy.node).collect();
    let (subjects, refusals) = group
        .take_copies_for_delete(&index, &selected, &denylist)
        .expect("copies are measured");
    assert!(refusals.is_empty(), "{refusals:?}");
    let expected: Vec<PathBuf> = selected.iter().map(|copy| index.path(*copy)).collect();
    let taken: Vec<PathBuf> = subjects.iter().map(|subject| subject.path.clone()).collect();
    assert_eq!(taken, expected);

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(report.deleted, subjects);
    assert_eq!(
        report.freed_bytes,
        subjects.iter().map(|subject| subject.measured_bytes).sum::<u64>()
    );
    assert!(index.physical_path(group.keep).exists());
    for copy in &selected {
        assert!(!index.physical_path(*copy).exists());
    }
}
