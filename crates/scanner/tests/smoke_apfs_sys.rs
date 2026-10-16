use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::os::fd::{AsFd, OwnedFd};

use domovoi_apfs_sys::{AttributeSet, BulkListing, EntryAttributes};
use domovoi_scanner::{NodeState, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

#[test]
fn scanner_nodes_carry_the_real_bulk_listing_attributes() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("original", 20_000, Fill::Noise(3)).expect("file is written");
    root.clone_of("original", "copy").expect("clone is created");
    root.directory("nested")
        .expect("directory is created")
        .file("inner", 10, Fill::Byte(9))
        .expect("nested file is written");

    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    let handle = OwnedFd::from(File::open(tree.path()).expect("tree opens"));
    let listed: BTreeMap<OsString, EntryAttributes> = BulkListing::new(handle.as_fd(), AttributeSet::Extended)
        .map(|entry| {
            let entry = entry.expect("entry parses");
            (entry.name, entry.attributes.expect("entry has attributes"))
        })
        .collect();

    assert_eq!(report.root.children.len(), listed.len());
    for child in &report.root.children {
        assert_eq!(
            child.attributes.as_ref(),
            Some(&listed[&child.name]),
            "{:?}",
            child.name
        );
    }
    let nested = report
        .root
        .children
        .iter()
        .find(|child| child.name == "nested")
        .expect("nested directory is listed");
    assert_eq!(nested.state, NodeState::Listed);
    assert_eq!(nested.children.len(), 1);
    assert_eq!(nested.children[0].state, NodeState::Leaf);
}
