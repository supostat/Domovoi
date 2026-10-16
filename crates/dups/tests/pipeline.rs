use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use domovoi_apfs_sys::sys::IOPOL_UTILITY;
use domovoi_apfs_sys::thread_disk_policy;
use domovoi_dups::{
    CloudFolders, DuplicateOptions, DuplicateStage, Duplicates, HashingPool, SearchError, SkipReason, find_duplicates,
};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, Reclaimable};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const MEBIBYTE: u64 = 1024 * 1024;
const PROGRESS_CAPACITY: usize = 4096;

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind is known"),
        &Enrichment::default(),
    )
}

fn node(index: &Index, relative: &str) -> NodeId {
    let mut current = index.root();
    for name in Path::new(relative).components() {
        current = index
            .child_named(current, name.as_os_str())
            .unwrap_or_else(|| panic!("{relative} is indexed"));
    }
    current
}

fn options() -> DuplicateOptions {
    DuplicateOptions::new(CloudFolders::new(Vec::new()))
}

fn search(index: &Index, options: &DuplicateOptions) -> Duplicates {
    let pool = HashingPool::new(None).expect("hashing pool starts");
    find_duplicates(index, &pool, options).expect("search completes")
}

fn overwrite_middle(path: &Path, size: u64) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .expect("file opens for writing")
        .write_all_at(b"a different middle", size / 2)
        .expect("middle is overwritten");
}

fn names(index: &Index, nodes: impl IntoIterator<Item = NodeId>) -> BTreeSet<String> {
    nodes
        .into_iter()
        .map(|node| {
            index
                .path(node)
                .strip_prefix(index.path(index.root()))
                .expect("node lies under the root")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn identical_files_form_a_group_and_only_independent_copies_count_as_gain() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let photos = root.directory("photos").expect("directory is created");
    photos
        .file("a.raw", 2 * MEBIBYTE, Fill::Noise(1))
        .expect("file is written");
    photos.hardlink("a.raw", "a-link.raw").expect("hard link is created");
    photos.clone_of("a.raw", "a-clone.raw").expect("clone is created");
    photos
        .file("b.raw", 2 * MEBIBYTE, Fill::Noise(2))
        .expect("file is written");
    photos
        .file("edges.raw", 2 * MEBIBYTE, Fill::Noise(1))
        .expect("file is written");
    overwrite_middle(&photos.path().join("edges.raw"), 2 * MEBIBYTE);
    root.directory("backup")
        .expect("directory is created")
        .file("a.raw", 2 * MEBIBYTE, Fill::Noise(1))
        .expect("file is written");
    root.directory("old")
        .expect("directory is created")
        .file("a copy.raw", 2 * MEBIBYTE, Fill::Noise(1))
        .expect("file is written");
    root.directory("project")
        .expect("directory is created")
        .directory("node_modules")
        .expect("directory is created")
        .file("a.raw", 2 * MEBIBYTE, Fill::Noise(1))
        .expect("file is written");
    let small = root.directory("small").expect("directory is created");
    for name in ["x", "y"] {
        small.file(name, MEBIBYTE / 2, Fill::Noise(3)).expect("file is written");
    }
    let index = index_of(tree.path());
    let (sender, receiver) = mpsc::sync_channel(PROGRESS_CAPACITY);
    let options = DuplicateOptions {
        progress: Some(sender),
        ..options()
    };

    let duplicates = search(&index, &options);

    assert_eq!(duplicates.groups.len(), 1, "{duplicates:?}");
    let group = &duplicates.groups[0];
    assert_eq!(group.size, 2 * MEBIBYTE);
    assert!(group.fully_hashed);
    let members: BTreeSet<String> = names(
        &index,
        std::iter::once(group.keep).chain(group.copies.iter().map(|copy| copy.node)),
    );
    assert_eq!(
        members,
        BTreeSet::from(
            [
                "photos/a.raw",
                "photos/a-link.raw",
                "photos/a-clone.raw",
                "backup/a.raw",
                "old/a copy.raw",
                "project/node_modules/a.raw",
            ]
            .map(String::from)
        )
    );
    let keep = names(&index, [group.keep]).into_iter().next().expect("one keeper");
    assert!(keep.starts_with("photos/a"), "{keep}");
    let sharing = names(
        &index,
        group
            .copies
            .iter()
            .filter(|copy| copy.shares_blocks)
            .map(|copy| copy.node),
    );
    let photos_set: BTreeSet<String> = ["photos/a.raw", "photos/a-link.raw", "photos/a-clone.raw"]
        .map(String::from)
        .into_iter()
        .filter(|name| *name != keep)
        .collect();
    assert_eq!(sharing, photos_set);
    let independent: Vec<NodeId> = group
        .copies
        .iter()
        .filter(|copy| !copy.shares_blocks)
        .map(|copy| copy.node)
        .collect();
    assert_eq!(
        names(&index, independent.iter().copied()),
        BTreeSet::from(["backup/a.raw", "old/a copy.raw", "project/node_modules/a.raw"].map(String::from))
    );
    let mut expected = Reclaimable::default();
    for copy in &independent {
        expected += index.reclaimable(*copy).expect("reclaimable is read");
    }
    assert_eq!(group.reclaimable, expected);

    let progress: Vec<_> = receiver.try_iter().collect();
    let stages: BTreeSet<_> = progress.iter().map(|event| format!("{:?}", event.stage)).collect();
    assert_eq!(stages, BTreeSet::from(["Contents".to_owned(), "Edges".to_owned()]));
    assert!(progress.iter().all(|event| event.files_done <= event.files_total));
}

#[test]
fn files_above_the_full_hash_limit_are_grouped_by_size_and_edges_only() {
    let tree = TempTree::new().expect("temp tree is created");
    let big = tree
        .root()
        .expect("root opens")
        .directory("big")
        .expect("directory is created");
    for name in ["one", "two"] {
        big.file(name, 3 * MEBIBYTE, Fill::Noise(5)).expect("file is written");
    }
    overwrite_middle(&big.path().join("two"), 3 * MEBIBYTE);
    let index = index_of(tree.path());

    assert!(search(&index, &options()).groups.is_empty());

    let limited = DuplicateOptions {
        full_hash_limit: 2 * MEBIBYTE,
        ..options()
    };
    let duplicates = search(&index, &limited);
    assert_eq!(duplicates.groups.len(), 1);
    assert!(!duplicates.groups[0].fully_hashed);
    assert_eq!(duplicates.groups[0].copies.len(), 1);
}

#[test]
fn cloud_folders_are_hashed_last_within_their_time_limit() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    for folder in ["cloud", "local"] {
        root.directory(folder)
            .expect("directory is created")
            .file("doc.pages", 2 * MEBIBYTE, Fill::Noise(7))
            .expect("file is written");
    }
    let index = index_of(tree.path());
    let cloud_folders = CloudFolders::new(vec![index.path(node(&index, "cloud"))]);

    let expired = DuplicateOptions {
        cloud_time_limit: Duration::ZERO,
        ..DuplicateOptions::new(cloud_folders.clone())
    };
    let duplicates = search(&index, &expired);
    assert!(duplicates.groups.is_empty());
    assert!(
        duplicates
            .skipped
            .iter()
            .any(|skipped| skipped.node == node(&index, "cloud/doc.pages") && skipped.reason == SkipReason::TimeLimit),
        "{:?}",
        duplicates.skipped
    );

    let (sender, receiver) = mpsc::sync_channel(PROGRESS_CAPACITY);
    let generous = DuplicateOptions {
        progress: Some(sender),
        ..DuplicateOptions::new(cloud_folders)
    };
    let duplicates = search(&index, &generous);
    assert_eq!(duplicates.groups.len(), 1);
    assert!(
        receiver
            .try_iter()
            .any(|event| event.stage == DuplicateStage::CloudFolders)
    );
}

#[test]
fn hashing_runs_on_utility_threads_and_stops_when_cancelled() {
    let pool = HashingPool::new(None).expect("hashing pool starts");
    assert_eq!(
        pool.install(|| thread_disk_policy().expect("thread policy is read")),
        IOPOL_UTILITY
    );

    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    for name in ["first", "second"] {
        root.file(name, 2 * MEBIBYTE, Fill::Noise(9)).expect("file is written");
    }
    let index = index_of(tree.path());
    let cancelled = options();
    cancelled.cancel.cancel();
    assert!(matches!(
        find_duplicates(&index, &pool, &cancelled),
        Err(SearchError::Cancelled)
    ));
}
