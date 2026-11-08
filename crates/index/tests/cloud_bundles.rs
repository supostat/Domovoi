use std::path::Path;

use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_scanner::{Node, NodeState, ScanOptions, scan};
use domovoi_testkit::{TempTree, application_bundle};

const ON_DISK: (&str, &str) = ("OnDisk", "com.domovoi.on-disk");
const PACKAGE_IN_CLOUD: (&str, &str) = ("PackageInCloud", "com.domovoi.package-in-cloud");
const INFO_IN_CLOUD: (&str, &str) = ("InfoInCloud", "com.domovoi.info-in-cloud");

fn scanned<'tree>(root: &'tree mut Node, relative: &str) -> &'tree mut Node {
    Path::new(relative).iter().fold(root, |node, name| {
        node.children
            .iter_mut()
            .find(|child| child.name.as_os_str() == name)
            .unwrap_or_else(|| panic!("{relative} is scanned"))
    })
}

#[test]
fn a_bundle_whose_package_or_info_plist_is_in_the_cloud_is_not_opened() {
    let tree = TempTree::new().expect("a temp tree is created");
    for (name, bundle_id) in [ON_DISK, PACKAGE_IN_CLOUD, INFO_IN_CLOUD] {
        application_bundle(tree.path(), name, bundle_id, &[]).expect("a bundle is written");
    }
    let mut report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
    let package = scanned(&mut report.root, &format!("{}.app", PACKAGE_IN_CLOUD.0));
    package.state = NodeState::InCloud;
    package.children.clear();
    scanned(
        &mut report.root,
        &format!("{}.app/Contents/Info.plist", INFO_IN_CLOUD.0),
    )
    .state = NodeState::InCloud;

    let index = Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("the root kind is known"),
        &Enrichment::default(),
    );

    assert_eq!(
        index
            .bundles()
            .iter()
            .map(|bundle| bundle.bundle_id.as_str())
            .collect::<Vec<_>>(),
        [ON_DISK.1],
        "only a bundle whose Info.plist is on disk is read; the files on disk of the others stay untouched"
    );
}
