use std::fs;
use std::path::Path;

use domovoi_dups::{CloudFolders, DuplicateAction, DuplicateOptions, HashingPool, find_duplicates};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_rules::{CaseRule, Denylist, TrashJournal};
use domovoi_scanner::{ScanOptions, scan};
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
fn duplicate_copies_go_to_the_trash_of_domovoi_by_default() {
    let tree = TempTree::new().expect("temp tree is created");
    let workspace = TempTree::new().expect("journal workspace is created");
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
    assert!(group.copies.iter().all(|copy| !copy.shares_blocks));
    assert_eq!(DuplicateAction::default(), DuplicateAction::MoveToTrash);

    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let mut journal = TrashJournal::open(&workspace.path().join("journal")).expect("journal opens");
    let selected: Vec<NodeId> = group.copies.iter().map(|copy| copy.node).collect();
    let outcomes = group
        .move_copies_to_trash(&index, &selected, &denylist, &mut journal)
        .expect("batch runs");

    assert!(outcomes.iter().all(|outcome| outcome.result.is_ok()), "{outcomes:?}");
    assert!(index.physical_path(group.keep).exists());
    for copy in &selected {
        assert!(!index.physical_path(*copy).exists());
    }
    let journaled: Vec<String> = journal.entries().iter().map(|entry| entry.original.clone()).collect();
    let expected: Vec<String> = selected
        .iter()
        .map(|copy| index.path(*copy).to_string_lossy().into_owned())
        .collect();
    assert_eq!(journaled, expected);

    while !journal.entries().is_empty() {
        let restored = journal.restore(0).expect("copy comes back");
        assert_eq!(
            fs::read(&restored).expect("restored copy is read"),
            fs::read(index.physical_path(group.keep)).expect("kept file is read")
        );
    }
}
