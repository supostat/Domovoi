use std::fs;
use std::path::Path;
use std::process::Command;

use domovoi_apfs_sys::private_size;
use domovoi_index::{Enrichment, HeldGroup, HeldKey, Index, IndexRoot, NodeId, Reclaimable};
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

fn inside(index: &Index, folder: NodeId, name: &str) -> NodeId {
    index
        .child_named(folder, name.as_ref())
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

fn freed(bytes: u64) -> Reclaimable {
    Reclaimable {
        bytes,
        ..Reclaimable::default()
    }
}

fn held_by_clones(bytes: u64) -> Reclaimable {
    Reclaimable {
        held_by_clones: bytes,
        ..Reclaimable::default()
    }
}

fn held_by_hard_links(bytes: u64) -> Reclaimable {
    Reclaimable {
        held_by_hard_links: bytes,
        ..Reclaimable::default()
    }
}

fn the_only_group(index: &Index, folder: NodeId) -> HeldGroup {
    let groups = index.held_groups(folder);
    assert_eq!(groups.len(), 1, "{groups:?}");
    groups[0]
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
    assert_eq!(index.reclaimable_estimate(packed), freed(index.node(packed).alloc_size));
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
        let packed = node(&index, name);
        let reclaimable = index.reclaimable(packed).expect("reclaimable is computed");
        assert_eq!(reclaimable.bytes, 0, "{name}");
        assert!(reclaimable.held_by_clones > 0, "{name}");
        assert_eq!(
            index.reclaimable_estimate(packed),
            held_by_clones(index.node(packed).alloc_size),
            "{name}"
        );
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
fn the_estimate_of_a_cloned_regular_file_is_held_by_clones_whole() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("original", 200_000, Fill::Noise(3)).expect("file is written");
    root.clone_of("original", "copy").expect("clone is created");
    let index = index_of(&tree);
    for name in ["original", "copy"] {
        let member = node(&index, name);
        assert_eq!(
            index.reclaimable_estimate(member),
            held_by_clones(index.node(member).alloc_size),
            "{name}"
        );
    }
}

#[test]
fn the_estimate_of_a_regular_file_frees_its_allocation_without_reading_the_disk() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .file("plain", 120_000, Fill::Noise(5))
        .expect("file is written");
    let index = index_of(&tree);
    let plain = node(&index, "plain");
    fs::remove_file(tree.path().join("plain")).expect("the file leaves the disk after indexing");
    assert_eq!(
        index.reclaimable(plain).expect("reclaimable is computed"),
        Reclaimable::default()
    );
    let estimate = index.reclaimable_estimate(plain);
    assert_eq!(estimate, freed(index.node(plain).alloc_size));
    assert_eq!(estimate.held_by_snapshots, 0);
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
    let allocated = index.node(link).alloc_size;
    let alone = index.reclaimable(link).expect("reclaimable is computed");
    assert_eq!((alone.bytes, alone.held_by_hard_links), (0, allocated));
    assert_eq!(
        index.reclaimable_estimate(link),
        Reclaimable {
            held_by_hard_links: allocated,
            ..Reclaimable::default()
        }
    );
    let together = index.reclaimable(directory).expect("reclaimable is computed");
    assert_eq!(together.held_by_hard_links, 0);
    assert_eq!(together.bytes, allocated);
    assert_eq!(index.reclaimable_estimate(directory), freed(allocated));
}

#[test]
fn a_hard_link_split_between_two_folders_is_freed_only_by_both_together() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.directory("alpha")
        .expect("directory is created")
        .file("shared", 100_000, Fill::Noise(4))
        .expect("file is written");
    root.directory("beta").expect("directory is created");
    fs::hard_link(tree.path().join("alpha/shared"), tree.path().join("beta/shared"))
        .expect("the folders share a hard link");
    let index = index_of(&tree);
    let (alpha, beta) = (node(&index, "alpha"), node(&index, "beta"));
    let allocated = index.node(inside(&index, alpha, "shared")).alloc_size;
    for folder in [alpha, beta] {
        assert_eq!(index.reclaimable_estimate(folder), held_by_hard_links(allocated));
        assert_eq!(
            index.reclaimable(folder).expect("reclaimable is computed"),
            held_by_hard_links(allocated)
        );
        let group = the_only_group(&index, folder);
        assert!(matches!(group.key, HeldKey::HardLinks { .. }), "{group:?}");
        assert_eq!(
            (group.bytes, group.members_inside, group.members_total),
            (allocated, 1, 2)
        );
    }
    assert_eq!(the_only_group(&index, alpha).key, the_only_group(&index, beta).key);
    assert_eq!(index.reclaimable_estimate_of(&[alpha, beta]), freed(allocated));
    assert_eq!(
        index.reclaimable_of(&[alpha, beta]).expect("reclaimable is computed"),
        freed(allocated)
    );
}

#[test]
fn a_clone_pair_reports_a_clone_refcount_of_two() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("original", 200_000, Fill::Noise(3)).expect("file is written");
    root.directory("elsewhere").expect("directory is created");
    clone_with_cp(tree.path(), "original", "elsewhere/copy");
    let index = index_of(&tree);
    let original = node(&index, "original");
    let copy = inside(&index, node(&index, "elsewhere"), "copy");
    assert_eq!(index.node(original).clone_refcount, 2);
    assert_eq!(index.node(copy).clone_refcount, 2);
}

#[test]
fn a_clone_split_between_two_folders_is_freed_only_by_both_together() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.directory("alpha")
        .expect("directory is created")
        .file("original", 200_000, Fill::Noise(3))
        .expect("file is written");
    root.directory("beta").expect("directory is created");
    clone_with_cp(tree.path(), "alpha/original", "beta/copy");
    let index = index_of(&tree);
    let (alpha, beta) = (node(&index, "alpha"), node(&index, "beta"));
    let allocated = index.node(inside(&index, alpha, "original")).alloc_size;
    for (folder, name) in [(alpha, "alpha/original"), (beta, "beta/copy")] {
        assert_eq!(index.reclaimable_estimate(folder), held_by_clones(allocated), "{name}");
        let private = private_size(&tree.path().join(name)).expect("private size is read");
        assert_eq!(
            index.reclaimable(folder).expect("reclaimable is computed"),
            Reclaimable {
                bytes: private,
                held_by_clones: allocated - private,
                ..Reclaimable::default()
            },
            "{name}"
        );
        let group = the_only_group(&index, folder);
        assert!(matches!(group.key, HeldKey::Clones { .. }), "{group:?}");
        assert_eq!(
            (group.bytes, group.members_inside, group.members_total),
            (allocated, 1, 2),
            "{name}"
        );
    }
    assert_eq!(the_only_group(&index, alpha).key, the_only_group(&index, beta).key);
    assert_eq!(index.reclaimable_estimate_of(&[alpha, beta]), freed(allocated));
    assert_eq!(
        index.reclaimable_of(&[alpha, beta]).expect("reclaimable is computed"),
        freed(allocated)
    );
}

#[test]
fn a_clone_pair_inside_one_folder_is_freed_once_by_the_folder() {
    let tree = TempTree::new().expect("temp tree is created");
    let folder = tree
        .root()
        .expect("root opens")
        .directory("pair")
        .expect("directory is created");
    folder
        .file("original", 200_000, Fill::Noise(3))
        .expect("file is written");
    folder.clone_of("original", "copy").expect("clone is created");
    let index = index_of(&tree);
    let pair = node(&index, "pair");
    let allocated = index.node(inside(&index, pair, "original")).alloc_size;
    assert_eq!(index.reclaimable_estimate(pair), freed(allocated));
    assert_eq!(
        index.reclaimable(pair).expect("reclaimable is computed"),
        freed(allocated)
    );
    assert!(index.held_groups(pair).is_empty());
}

#[test]
fn a_partner_outside_the_set_keeps_its_group_held() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let alpha = root.directory("alpha").expect("directory is created");
    alpha.file("linked", 100_000, Fill::Noise(4)).expect("file is written");
    alpha.file("cloned", 200_000, Fill::Noise(5)).expect("file is written");
    root.directory("beta")
        .expect("directory is created")
        .file("plain", 50_000, Fill::Noise(6))
        .expect("file is written");
    root.directory("gamma").expect("directory is created");
    fs::hard_link(tree.path().join("alpha/linked"), tree.path().join("gamma/linked"))
        .expect("the partner link lies outside the set");
    clone_with_cp(tree.path(), "alpha/cloned", "gamma/cloned");
    let index = index_of(&tree);
    let (alpha, beta) = (node(&index, "alpha"), node(&index, "beta"));
    let allocated = |name: &str| index.node(inside(&index, alpha, name)).alloc_size;
    let plain = index.node(inside(&index, beta, "plain")).alloc_size;
    assert_eq!(
        index.reclaimable_estimate_of(&[alpha, beta]),
        Reclaimable {
            bytes: plain,
            held_by_clones: allocated("cloned"),
            held_by_hard_links: allocated("linked"),
            ..Reclaimable::default()
        }
    );
    let exact = index.reclaimable_of(&[alpha, beta]).expect("reclaimable is computed");
    let private = private_size(&tree.path().join("alpha/cloned")).expect("private size is read");
    assert_eq!(exact.held_by_hard_links, allocated("linked"));
    assert_eq!(exact.held_by_clones, allocated("cloned") - private);
}

#[test]
fn a_member_inside_another_member_of_the_set_counts_once() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .directory("folder")
        .expect("directory is created")
        .file("plain", 120_000, Fill::Noise(5))
        .expect("file is written");
    let index = index_of(&tree);
    let folder = node(&index, "folder");
    let plain = inside(&index, folder, "plain");
    assert_eq!(
        index.reclaimable_estimate_of(&[plain, folder, plain]),
        index.reclaimable_estimate(folder)
    );
    assert_eq!(
        index.reclaimable_of(&[folder, plain]).expect("reclaimable is computed"),
        index.reclaimable(folder).expect("reclaimable is computed")
    );
}

#[test]
fn a_compressed_clone_pair_stays_held_because_decmpfs_files_report_no_clone_refcount() {
    let tree = TempTree::new().expect("temp tree is created");
    let folder = tree
        .root()
        .expect("root opens")
        .directory("pair")
        .expect("directory is created");
    folder
        .compressed("packed", 256 * 1024)
        .expect("compressed file is created");
    clone_with_cp(&tree.path().join("pair"), "packed", "packed-copy");
    let index = index_of(&tree);
    let pair = node(&index, "pair");
    let members = ["packed", "packed-copy"].map(|name| inside(&index, pair, name));
    assert_eq!(members.map(|member| index.node(member).clone_refcount), [0, 0]);
    let allocated = members.iter().map(|member| index.node(*member).alloc_size).sum();
    assert_eq!(index.reclaimable_estimate(pair), held_by_clones(allocated));
    assert!(index.held_groups(pair).is_empty());
}
