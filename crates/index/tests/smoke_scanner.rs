use std::path::Path;
use std::process::Command;

use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

fn du_bytes(path: &Path) -> u64 {
    let output = Command::new("/usr/bin/du")
        .arg("-sk")
        .arg(path)
        .output()
        .expect("du runs");
    assert!(
        output.status.success(),
        "du failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let kibibytes: u64 = String::from_utf8(output.stdout)
        .expect("du prints ASCII")
        .split_whitespace()
        .next()
        .and_then(|value| value.parse().ok())
        .expect("du prints a size");
    kibibytes * 1024
}

#[test]
fn the_real_scanner_feeds_an_index_whose_inode_total_matches_du() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("noise", 500_000, Fill::Noise(11)).expect("file is written");
    root.hardlink("noise", "noise-link").expect("link is created");
    root.clone_of("noise", "noise-clone").expect("clone is created");
    root.sparse("sparse", 20_000_000).expect("sparse file is created");
    root.compressed("packed", 300 * 1024)
        .expect("compressed file is created");
    root.pnpm_store(2).expect("pnpm store is created");
    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    let index = Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        &Enrichment::default(),
    );
    let inode_total = index.unique_inode_allocation();
    assert_eq!(inode_total, du_bytes(tree.path()));
    let deduplicated = index.node(index.root()).allocated;
    assert!(
        deduplicated < inode_total,
        "clones are counted once: {deduplicated} < {inode_total}"
    );
}
