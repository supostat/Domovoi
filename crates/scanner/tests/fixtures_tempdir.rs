use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::Duration;

use domovoi_apfs_sys::ObjectKind;
use domovoi_apfs_sys::sys::{EF_SHARES_ALL_BLOCKS, UF_COMPRESSED};
use domovoi_scanner::{Node, NodeState, Onboarding, ProtectedFolders, ScanError, ScanOptions, ScanProgress, scan};
use domovoi_testkit::{Fill, TempTree};

const SIGNAL_DELAY: Duration = Duration::from_millis(200);

fn scan_tree(tree: &TempTree) -> Node {
    scan(tree.path(), &ScanOptions::default())
        .expect("temp tree scans")
        .root
}

fn child<'node>(node: &'node Node, name: &str) -> &'node Node {
    node.children
        .iter()
        .find(|child| child.name == name)
        .unwrap_or_else(|| panic!("{name} is listed"))
}

fn every_node(node: &Node) -> Vec<&Node> {
    let mut nodes = vec![node];
    for child in &node.children {
        nodes.extend(every_node(child));
    }
    nodes
}

fn unique_inode_allocation(root: &Node) -> u64 {
    let mut by_inode = BTreeMap::new();
    for node in every_node(root) {
        if let Some(attributes) = &node.attributes {
            by_inode.insert(
                (attributes.real_dev, attributes.file_id),
                attributes.alloc_size.unwrap_or(0),
            );
        }
    }
    by_inode.values().sum()
}

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

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("path canonicalizes")
}

#[test]
fn hard_links_share_one_inode() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("original", 40_000, Fill::Noise(1)).expect("file is written");
    root.hardlink("original", "link-a").expect("link is created");
    root.hardlink("original", "link-b").expect("link is created");
    let scanned = scan_tree(&tree);
    let ids: Vec<_> = ["original", "link-a", "link-b"]
        .map(|name| child(&scanned, name).attributes.clone().expect("attributes"))
        .into_iter()
        .map(|attributes| (attributes.file_id, attributes.link_count))
        .collect();
    assert!(ids.iter().all(|id| *id == ids[0]));
    assert_eq!(ids[0].1, Some(3));
    let allocation = child(&scanned, "original")
        .attributes
        .as_ref()
        .and_then(|attributes| attributes.alloc_size);
    assert_eq!(Some(unique_inode_allocation(&scanned)), allocation);
}

#[test]
fn clones_share_a_clone_id_and_all_blocks() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("original", 40_000, Fill::Noise(2)).expect("file is written");
    root.clone_of("original", "clone").expect("clone is created");
    let scanned = scan_tree(&tree);
    let original = child(&scanned, "original").attributes.clone().expect("attributes");
    let clone = child(&scanned, "clone").attributes.clone().expect("attributes");
    assert_ne!(original.file_id, clone.file_id);
    assert_eq!(original.clone_id, clone.clone_id);
    assert_eq!(clone.clone_refcount, Some(2));
    assert!(
        clone
            .ext_flags
            .is_some_and(|flags| flags & u64::from(EF_SHARES_ALL_BLOCKS) != 0)
    );
}

#[test]
fn sparse_and_decmpfs_files_report_their_real_allocation() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.sparse("sparse", 50_000_000).expect("sparse file is created");
    root.compressed("packed", 256 * 1024)
        .expect("compressed file is created");
    let scanned = scan_tree(&tree);
    let sparse = child(&scanned, "sparse").attributes.clone().expect("attributes");
    assert_eq!((sparse.total_size, sparse.alloc_size), (Some(50_000_000), Some(0)));
    let packed = child(&scanned, "packed").attributes.clone().expect("attributes");
    assert_ne!(packed.flags & UF_COMPRESSED, 0);
    assert!(packed.alloc_size < packed.total_size);
}

#[test]
fn unreadable_directories_belong_to_another_user() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.directory("closed").expect("directory is created");
    root.directory("search-only").expect("directory is created");
    root.mode("closed", 0o000).expect("mode is set");
    root.mode("search-only", 0o100).expect("mode is set");
    let scanned = scan_tree(&tree);
    assert_eq!(child(&scanned, "closed").state, NodeState::OtherUser);
    assert_eq!(child(&scanned, "search-only").state, NodeState::OtherUser);
}

#[test]
fn symlink_loops_stay_unresolved_leaves() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .symlink_loop("loop")
        .expect("loop is created");
    let scanned = scan_tree(&tree);
    for name in ["loop", "loop.partner"] {
        let node = child(&scanned, name);
        assert_eq!(node.state, NodeState::Leaf);
        assert_eq!(
            node.attributes.as_ref().map(|attributes| attributes.kind),
            Some(ObjectKind::Symlink)
        );
    }
}

#[test]
fn chains_deeper_than_path_max_are_walked_by_descriptor() {
    let tree = TempTree::new().expect("temp tree is created");
    let deepest = tree.root().expect("root opens").deep(3).expect("deep chain is created");
    deepest
        .file("bottom", 64, Fill::Byte(5))
        .expect("bottom file is written");
    let scanned = scan_tree(&tree);
    let mut node = &scanned;
    for _ in 0..3 {
        assert_eq!(node.children.len(), 1);
        node = &node.children[0];
        assert_eq!(node.state, NodeState::Listed);
    }
    assert_eq!(child(node, "bottom").state, NodeState::Leaf);
}

#[test]
fn unique_inode_allocation_matches_du() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("noise", 300_000, Fill::Noise(4)).expect("file is written");
    root.hardlink("noise", "noise-link").expect("link is created");
    root.clone_of("noise", "noise-clone").expect("clone is created");
    root.sparse("sparse", 10_000_000).expect("sparse file is created");
    root.compressed("packed", 512 * 1024)
        .expect("compressed file is created");
    root.symlink_loop("loop").expect("loop is created");
    root.pnpm_store(3).expect("pnpm store is created");
    root.directory("nested")
        .expect("directory is created")
        .file("inner", 9_000, Fill::Noise(5))
        .expect("nested file is written");
    let scanned = scan_tree(&tree);
    assert_eq!(unique_inode_allocation(&scanned), du_bytes(tree.path()));
}

#[test]
fn progress_reports_firmlink_paths_and_ends_with_the_walked_totals() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("one", 8_192, Fill::Byte(1)).expect("file is written");
    root.directory("inner")
        .expect("directory is created")
        .file("two", 8_192, Fill::Byte(2))
        .expect("file is written");
    let (sender, receiver) = sync_channel(64);
    let options = ScanOptions {
        progress: Some(sender),
        ..ScanOptions::default()
    };
    let report = scan(tree.path(), &options).expect("temp tree scans");
    drop(options);
    let events: Vec<_> = receiver.iter().collect();
    assert_eq!(
        events.last(),
        Some(&ScanProgress::Walked {
            files: report.files,
            bytes: report.bytes
        })
    );
    assert_eq!(report.files, 2);
    let root_path = canonical(tree.path());
    let walking = &events[..events.len() - 1];
    assert!(!walking.is_empty(), "the walk reports progress before it ends");
    assert!(
        walking
            .iter()
            .all(|event| matches!(event, ScanProgress::Walking { current, .. } if current.starts_with(&root_path)))
    );
}

#[test]
fn cancelled_scan_returns_cancelled() {
    let tree = TempTree::new().expect("temp tree is created");
    let options = ScanOptions::default();
    options.cancel.cancel();
    assert!(matches!(scan(tree.path(), &options), Err(ScanError::Cancelled)));
}

#[test]
fn protected_folders_wait_for_the_end_of_onboarding() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .directory("Documents")
        .expect("directory is created")
        .file("letter", 100, Fill::Byte(6))
        .expect("file is written");
    let onboarding = Arc::new(Onboarding::default());
    let options = ScanOptions {
        protected: ProtectedFolders::new(vec![canonical(tree.path()).join("Documents")]),
        onboarding: Arc::clone(&onboarding),
        ..ScanOptions::default()
    };
    let finisher = thread::spawn(move || {
        thread::sleep(SIGNAL_DELAY);
        assert!(!onboarding.is_finished());
        onboarding.finish();
    });
    let scanned = scan(tree.path(), &options).expect("temp tree scans").root;
    finisher.join().expect("finisher thread completes");
    let documents = child(&scanned, "Documents");
    assert_eq!(documents.state, NodeState::Listed);
    assert_eq!(child(documents, "letter").state, NodeState::Leaf);
}

#[test]
fn protected_folders_are_scanned_last_even_after_onboarding() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.directory("Downloads")
        .expect("directory is created")
        .file("installer.dmg", 4_096, Fill::Byte(7))
        .expect("file is written");
    root.directory("Projects")
        .expect("directory is created")
        .file("notes.txt", 4_096, Fill::Byte(8))
        .expect("file is written");
    let (sender, receiver) = sync_channel(64);
    let options = ScanOptions {
        protected: ProtectedFolders::new(vec![canonical(tree.path()).join("Downloads")]),
        progress: Some(sender),
        ..ScanOptions::default()
    };
    let scanned = scan(tree.path(), &options).expect("temp tree scans").root;
    drop(options);
    let visited: Vec<_> = receiver
        .iter()
        .filter_map(|event| match event {
            ScanProgress::Walking { current, .. } => Some(current),
            ScanProgress::Walked { .. } => None,
        })
        .collect();
    assert_eq!(visited.last(), Some(&canonical(tree.path()).join("Downloads")));
    assert_eq!(child(&scanned, "Downloads").state, NodeState::Listed);
}

#[test]
fn waiting_for_onboarding_honours_cancellation() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .directory("Desktop")
        .expect("directory is created");
    let options = ScanOptions {
        protected: ProtectedFolders::new(vec![canonical(tree.path()).join("Desktop")]),
        onboarding: Arc::new(Onboarding::default()),
        ..ScanOptions::default()
    };
    let cancel = options.cancel.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(SIGNAL_DELAY);
        cancel.cancel();
    });
    assert!(matches!(scan(tree.path(), &options), Err(ScanError::Cancelled)));
    canceller.join().expect("canceller thread completes");
}
