use std::fs;
use std::path::Path;
use std::process::Command;

use domovoi_apfs_sys::AttributeSet;
use domovoi_index::{Enrichment, Index, IndexRoot, Reclaimable, measure_reclaimable};
use domovoi_scanner::{Node, ScanOptions, ScanReport, scan};
use domovoi_testkit::{Fill, TempTree};

fn clone_file(source: &Path, target: &Path) {
    let status = Command::new("/bin/cp")
        .arg("-c")
        .arg(source)
        .arg(target)
        .status()
        .expect("cp runs");
    assert!(status.success(), "cp -c failed");
}

fn workshop() -> TempTree {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let item = root.directory("item").expect("directory is created");
    root.directory("outside").expect("directory is created");
    item.file("plain", 40_960, Fill::Noise(1)).expect("file is written");
    item.file("twin", 32_768, Fill::Noise(2)).expect("file is written");
    item.clone_of("twin", "twin-clone").expect("the clone is made inside");
    item.file("lonely", 24_576, Fill::Noise(3)).expect("file is written");
    item.file("linked", 16_384, Fill::Noise(4)).expect("file is written");
    item.hardlink("linked", "linked-again")
        .expect("the hard link is made inside");
    item.file("shared", 12_288, Fill::Noise(5)).expect("file is written");
    item.compressed("packed", 200_000)
        .expect("a compressed file is written");
    item.clone_of("packed", "packed-clone")
        .expect("the compressed file is cloned");
    item.directory("nested")
        .expect("directory is created")
        .file("deep", 8_192, Fill::Noise(6))
        .expect("file is written");
    clone_file(
        &tree.path().join("item/lonely"),
        &tree.path().join("outside/lonely-clone"),
    );
    fs::hard_link(tree.path().join("item/shared"), tree.path().join("outside/shared-link"))
        .expect("the hard link is made outside");
    tree
}

fn scanned(tree: &TempTree) -> (ScanReport, Index) {
    let options = ScanOptions {
        attributes: AttributeSet::ExtendedWithPrivateSize,
        ..ScanOptions::default()
    };
    let report = scan(tree.path(), &options).expect("the tree scans");
    let index = Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        &Enrichment::default(),
    );
    (report, index)
}

fn child<'node>(node: &'node Node, name: &str) -> &'node Node {
    node.children
        .iter()
        .find(|child| child.name == name)
        .unwrap_or_else(|| panic!("{name} is scanned"))
}

fn indexed(index: &Index, tree: &TempTree, relative: &str) -> Reclaimable {
    let node = index
        .find(&tree.path().join(relative))
        .unwrap_or_else(|| panic!("{relative} is indexed"));
    index.reclaimable(node).expect("the index measures the item")
}

#[test]
fn a_measured_folder_equals_the_index_measurement() {
    let tree = workshop();
    let (report, index) = scanned(&tree);
    let item = child(&report.root, "item");
    let measured = measure_reclaimable(item);
    assert_eq!(measured, indexed(&index, &tree, "item"));
    assert!(measured.bytes > 0, "{measured:?}");
    assert!(measured.held_by_clones > 0, "{measured:?}");
    assert!(measured.held_by_hard_links > 0, "{measured:?}");
}

#[test]
fn measured_files_equal_the_index_measurement() {
    let tree = workshop();
    let (report, index) = scanned(&tree);
    let item = child(&report.root, "item");
    for name in ["plain", "twin", "lonely", "linked", "shared", "packed", "packed-clone"] {
        assert_eq!(
            measure_reclaimable(child(item, name)),
            indexed(&index, &tree, &format!("item/{name}")),
            "{name}"
        );
    }
}
