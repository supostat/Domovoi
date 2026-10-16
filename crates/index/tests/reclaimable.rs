use std::path::Path;
use std::process::Command;

use domovoi_apfs_sys::private_size;
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

fn index_of(tree: &TempTree) -> Index {
    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        &Enrichment::default(),
    )
}

fn node(index: &Index, name: &str) -> NodeId {
    index
        .child_named(index.root(), name.as_ref())
        .unwrap_or_else(|| panic!("{name} is indexed"))
}

fn clone_with_cp(directory: &Path, source: &str, target: &str) {
    let status = Command::new("/bin/cp")
        .arg("-c")
        .arg(directory.join(source))
        .arg(directory.join(target))
        .status()
        .expect("cp runs");
    assert!(status.success(), "cp -c failed");
}

#[test]
fn a_lone_decmpfs_file_frees_its_allocation() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .compressed("packed", 256 * 1024)
        .expect("compressed file is created");
    let index = index_of(&tree);
    let packed = node(&index, "packed");
    let reclaimable = index.reclaimable(packed).expect("reclaimable is computed");
    assert_eq!(reclaimable.bytes, index.node(packed).alloc_size);
    assert_eq!(reclaimable.held_by_clones, 0);
}

#[test]
fn a_decmpfs_file_with_a_clone_frees_nothing() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .compressed("packed", 256 * 1024)
        .expect("compressed file is created");
    clone_with_cp(tree.path(), "packed", "packed-copy");
    let index = index_of(&tree);
    for name in ["packed", "packed-copy"] {
        let reclaimable = index.reclaimable(node(&index, name)).expect("reclaimable is computed");
        assert_eq!(reclaimable.bytes, 0, "{name}");
        assert!(reclaimable.held_by_clones > 0, "{name}");
    }
}

#[test]
fn a_cloned_regular_file_frees_its_private_size() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("original", 200_000, Fill::Noise(3)).expect("file is written");
    root.clone_of("original", "copy").expect("clone is created");
    let index = index_of(&tree);
    let original = node(&index, "original");
    let reclaimable = index.reclaimable(original).expect("reclaimable is computed");
    let private = private_size(&tree.path().join("original")).expect("private size is read");
    assert_eq!(reclaimable.bytes, private);
    assert_eq!(reclaimable.held_by_clones, index.node(original).alloc_size - private);
}

#[test]
fn one_of_several_hard_links_frees_nothing_but_all_of_them_do() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let shared = root.directory("shared").expect("directory is created");
    shared.file("a", 100_000, Fill::Noise(4)).expect("file is written");
    shared.hardlink("a", "b").expect("link is created");
    let index = index_of(&tree);
    let directory = node(&index, "shared");
    let link = index.child_named(directory, "a".as_ref()).expect("link is indexed");
    let alone = index.reclaimable(link).expect("reclaimable is computed");
    assert_eq!(
        (alone.bytes, alone.held_by_hard_links),
        (0, index.node(link).alloc_size)
    );
    let together = index.reclaimable(directory).expect("reclaimable is computed");
    assert_eq!(together.held_by_hard_links, 0);
    assert_eq!(together.bytes, index.node(link).alloc_size);
}
