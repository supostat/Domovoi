use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_dups::{CloudFolders, DuplicateGroup, DuplicateOptions, HashingPool, find_duplicates};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const SIZE: u64 = 2 * 1024 * 1024;
const KEPT: &str = "Documents/report.raw";
const COPY: &str = "Backup/report.raw";
const CLONE_OF_COPY: &str = "Old/report.raw";

fn volume_with_a_cloned_pair_of_copies() -> TempTree {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.root().expect("the root opens");
    for relative in [KEPT, COPY] {
        let path = Path::new(relative);
        root.directory(path.parent().expect("a file sits in a folder"))
            .expect("a folder is created")
            .file(path.file_name().expect("a file has a name"), SIZE, Fill::Noise(3))
            .expect("a file is written");
    }
    root.directory("Old").expect("a folder is created");
    let cloned = Command::new("/bin/cp")
        .arg("-c")
        .arg(tree.path().join(COPY))
        .arg(tree.path().join(CLONE_OF_COPY))
        .status()
        .expect("cp runs");
    assert!(cloned.success(), "cp -c clones the copy");
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
    index
        .find(&index.path(index.root()).join(relative))
        .unwrap_or_else(|| panic!("{relative} is indexed"))
}

fn the_group(index: &Index) -> DuplicateGroup {
    let pool = HashingPool::new(None).expect("the hashing pool starts");
    let groups = find_duplicates(index, &pool, &DuplicateOptions::new(CloudFolders::new(Vec::new())))
        .expect("the search completes")
        .groups;
    let [group] = groups.try_into().expect("one group is found");
    group
}

#[test]
fn copies_that_are_clones_only_of_each_other_can_go() {
    let tree = volume_with_a_cloned_pair_of_copies();
    let index = index_of(tree.path());

    let group = the_group(&index);

    assert_eq!(group.keep, node_at(&index, KEPT));
    let removable: BTreeMap<PathBuf, bool> = group
        .copies
        .iter()
        .map(|copy| (index.path(copy.node), copy.is_removable()))
        .collect();
    assert_eq!(
        removable,
        BTreeMap::from([
            (index.path(node_at(&index, COPY)), true),
            (index.path(node_at(&index, CLONE_OF_COPY)), true),
        ]),
        "a clone of another copy does not keep the kept file alive"
    );
}
