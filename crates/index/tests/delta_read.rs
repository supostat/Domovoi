use std::fs;
use std::path::{Path, PathBuf};

use domovoi_index::{ChangedPath, DirectoryListing, Enrichment, Index, IndexRoot, ListedEntry};
use domovoi_scanner::{Node, NodeState, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

fn project() -> TempTree {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("readme", 4_096, Fill::Byte(1)).expect("file is written");
    let first = root.directory("a").expect("directory is created");
    first.file("top", 4_096, Fill::Byte(2)).expect("file is written");
    first
        .directory("b")
        .expect("directory is created")
        .directory("c")
        .expect("directory is created")
        .file("deep", 4_096, Fill::Byte(3))
        .expect("file is written");
    root.directory("keep")
        .expect("directory is created")
        .directory("inner")
        .expect("directory is created")
        .file("kept", 4_096, Fill::Byte(4))
        .expect("file is written");
    tree
}

fn index_of(tree: &TempTree) -> Index {
    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        &Enrichment::default(),
    )
}

fn root_path(index: &Index) -> PathBuf {
    index.path(index.root())
}

fn change(path: PathBuf, subtree: bool) -> ChangedPath {
    ChangedPath { path, subtree }
}

fn targets(index: &Index, changed: &[ChangedPath]) -> Vec<(PathBuf, bool)> {
    index
        .delta_request(changed)
        .targets()
        .iter()
        .map(|target| (target.path().to_path_buf(), target.subtree()))
        .collect()
}

fn read_one(index: &Index, changed: ChangedPath) -> DirectoryListing {
    let request = index.delta_request(&[changed]);
    let [target] = request.targets() else {
        panic!("one target is expected: {request:?}");
    };
    target.read(&ScanOptions::default()).expect("the target reads")
}

fn entry<'listing>(listing: &'listing DirectoryListing, name: &str) -> &'listing ListedEntry {
    listing
        .entries
        .iter()
        .find(|entry| entry.node().name == name)
        .unwrap_or_else(|| panic!("{name} is listed: {listing:?}"))
}

fn child<'node>(node: &'node Node, name: &str) -> &'node Node {
    node.children
        .iter()
        .find(|child| child.name == name)
        .unwrap_or_else(|| panic!("{name} is a child of {}", node.name.display()))
}

fn under(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

#[test]
fn a_change_directly_in_the_root_targets_only_the_root() {
    let tree = project();
    let index = index_of(&tree);
    let root = root_path(&index);
    assert_eq!(targets(&index, &[change(root.clone(), false)]), [(root, false)]);
}

#[test]
fn nested_changed_directories_are_separate_targets() {
    let tree = project();
    let index = index_of(&tree);
    let root = root_path(&index);
    assert_eq!(
        targets(
            &index,
            &[change(under(&root, "a/b"), false), change(under(&root, "a"), false)]
        ),
        [(under(&root, "a"), false), (under(&root, "a/b"), false)]
    );
}

#[test]
fn a_subtree_target_absorbs_the_targets_inside_it() {
    let tree = project();
    let index = index_of(&tree);
    let root = root_path(&index);
    assert_eq!(
        targets(
            &index,
            &[change(under(&root, "a/b/c"), false), change(under(&root, "a"), true)]
        ),
        [(under(&root, "a"), true)]
    );
}

#[test]
fn a_vanished_directory_is_targeted_at_its_existing_ancestor() {
    let tree = project();
    let index = index_of(&tree);
    let root = root_path(&index);
    fs::remove_dir_all(tree.path().join("a/b")).expect("the folder is removed");
    assert_eq!(
        targets(&index, &[change(under(&root, "a/b/c"), false)]),
        [(under(&root, "a"), false)]
    );
}

#[test]
fn identical_targets_merge_and_keep_the_subtree_flag() {
    let tree = project();
    let index = index_of(&tree);
    let root = root_path(&index);
    assert_eq!(
        targets(
            &index,
            &[change(under(&root, "a"), false), change(under(&root, "a"), true)]
        ),
        [(under(&root, "a"), true)]
    );
    fs::create_dir(tree.path().join("a/new")).expect("a folder appears after indexing");
    assert_eq!(
        targets(&index, &[change(under(&root, "a/new"), true)]),
        [(under(&root, "a"), false)]
    );
}

#[test]
fn kept_directories_are_not_read_again() {
    let tree = project();
    let index = index_of(&tree);
    tree.root()
        .expect("root opens")
        .file("fresh", 4_096, Fill::Byte(5))
        .expect("file is written after indexing");
    let listing = read_one(&index, change(root_path(&index), false));
    assert_eq!(listing.descended, 0);
    for name in ["a", "keep"] {
        let ListedEntry::Kept(kept) = entry(&listing, name) else {
            panic!("{name} keeps its indexed subtree");
        };
        assert!(kept.children.is_empty());
    }
    assert!(matches!(entry(&listing, "fresh"), ListedEntry::Read(node) if node.state == NodeState::Leaf));
}

#[test]
fn new_and_replaced_directories_are_read_whole() {
    let tree = project();
    let index = index_of(&tree);
    let root = tree.root().expect("root opens");
    root.directory("added")
        .expect("directory is created")
        .directory("inner")
        .expect("directory is created")
        .file("x", 4_096, Fill::Byte(6))
        .expect("file is written");
    fs::remove_dir_all(tree.path().join("keep")).expect("the folder is removed");
    root.directory("keep")
        .expect("the folder is created again")
        .directory("other")
        .expect("directory is created")
        .file("y", 4_096, Fill::Byte(7))
        .expect("file is written");
    let listing = read_one(&index, change(root_path(&index), false));
    let ListedEntry::Read(added) = entry(&listing, "added") else {
        panic!("a new folder is read");
    };
    assert_eq!(child(child(added, "inner"), "x").state, NodeState::Leaf);
    let ListedEntry::Read(replaced) = entry(&listing, "keep") else {
        panic!("a folder with another FILEID is read");
    };
    assert_eq!(child(child(replaced, "other"), "y").state, NodeState::Leaf);
    assert!(matches!(entry(&listing, "a"), ListedEntry::Kept(_)));
    assert_eq!(listing.descended, 4);
}

#[test]
fn a_subtree_target_is_read_whole() {
    let tree = project();
    let index = index_of(&tree);
    let listing = read_one(&index, change(under(&root_path(&index), "a"), true));
    assert!(listing.subtree);
    assert!(
        listing
            .entries
            .iter()
            .all(|entry| matches!(entry, ListedEntry::Read(_)))
    );
    let ListedEntry::Read(nested) = entry(&listing, "b") else {
        panic!("the subtree is read");
    };
    assert_eq!(child(child(nested, "c"), "deep").state, NodeState::Leaf);
    assert_eq!(listing.descended, 2);
}
