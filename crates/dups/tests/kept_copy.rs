use std::path::Path;

use domovoi_dups::{CloudFolders, DuplicateGroup, DuplicateOptions, HashingPool, find_duplicates};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const SIZE: u64 = 2 * 1024 * 1024;

fn volume(relatives: &[&str]) -> TempTree {
    let tree = TempTree::new().expect("a temp tree is created");
    for relative in relatives {
        let path = Path::new(relative);
        let folder = path
            .parent()
            .expect("a file sits in a folder")
            .iter()
            .fold(tree.root().expect("the root opens"), |folder, name| {
                folder.directory(name).expect("a folder is created")
            });
        folder
            .file(path.file_name().expect("a file has a name"), SIZE, Fill::Noise(3))
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

fn groups(index: &Index) -> Vec<DuplicateGroup> {
    let pool = HashingPool::new(None).expect("the hashing pool starts");
    find_duplicates(index, &pool, &DuplicateOptions::new(CloudFolders::new(Vec::new())))
        .expect("the search completes")
        .groups
}

fn node_at(index: &Index, relative: &str) -> NodeId {
    index
        .find(&index.path(index.root()).join(relative))
        .unwrap_or_else(|| panic!("{relative} is indexed"))
}

#[test]
fn a_file_in_the_trash_is_never_the_kept_copy() {
    let tree = volume(&[".Trash/report.pdf", "Documents/report.pdf"]);
    let index = index_of(tree.path());

    let [group] = groups(&index).try_into().expect("one group is found");

    assert_eq!(
        group.keep,
        node_at(&index, "Documents/report.pdf"),
        "the trashed file is the copy even though it is older"
    );
}

#[test]
fn a_file_inside_a_package_is_kept_and_the_loose_copy_can_go() {
    let tree = volume(&["Downloads/Engine", "Slack.app/Contents/Frameworks/Engine"]);
    let index = index_of(tree.path());

    let [group] = groups(&index).try_into().expect("one group is found");

    assert_eq!(group.keep, node_at(&index, "Slack.app/Contents/Frameworks/Engine"));
    let [copy] = group.copies.as_slice() else {
        panic!("one copy: {:?}", group.copies);
    };
    assert_eq!(copy.node, node_at(&index, "Downloads/Engine"));
    assert!(copy.is_removable());
}

#[test]
fn a_copy_inside_a_package_is_never_removable_and_frees_nothing() {
    let tree = volume(&[
        "Downloads/Engine",
        "Slack.app/Contents/Frameworks/Engine",
        "Discord.app/Contents/Frameworks/Engine",
    ]);
    let index = index_of(tree.path());

    let [group] = groups(&index).try_into().expect("one group is found");

    let loose = node_at(&index, "Downloads/Engine");
    for copy in &group.copies {
        assert_eq!(copy.is_removable(), copy.node == loose, "{copy:?}");
    }
    assert_eq!(
        group.reclaimable,
        index.reclaimable(loose).expect("the loose copy is measured"),
        "only the loose copy counts"
    );
}

#[test]
fn a_group_whose_every_copy_is_inside_a_package_is_not_shown() {
    let tree = volume(&[
        "Slack.app/Contents/Frameworks/Engine",
        "Discord.app/Contents/Frameworks/Engine",
    ]);
    let index = index_of(tree.path());

    assert!(groups(&index).is_empty(), "nothing in the group can be removed");
}
