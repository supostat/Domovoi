use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use domovoi_apfs_sys::sys::IOPOL_UTILITY;
use domovoi_apfs_sys::thread_disk_policy;
use domovoi_dups::{
    CloudFolders, DuplicateOptions, DuplicateStage, Duplicates, HashingPool, SearchError, SkipReason, find_duplicates,
};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, Reclaimable};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const MEBIBYTE: u64 = 1024 * 1024;
const BEYOND_THE_OLD_FULL_HASH_LIMIT: u64 = 4 * 1024 * MEBIBYTE + MEBIBYTE;
const ENDLESS: u64 = 16 * 1024 * MEBIBYTE;
const STOP_AFTER: Duration = Duration::from_millis(500);
const STOPPED_WITHIN: Duration = Duration::from_secs(3);
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
fn files_of_any_size_are_compared_by_their_whole_contents() {
    let tree = TempTree::new().expect("temp tree is created");
    let big = tree
        .root()
        .expect("root opens")
        .directory("big")
        .expect("directory is created");
    for name in ["one", "two"] {
        big.sparse(name, BEYOND_THE_OLD_FULL_HASH_LIMIT)
            .expect("file is written");
    }
    overwrite_middle(&big.path().join("two"), BEYOND_THE_OLD_FULL_HASH_LIMIT);
    let index = index_of(tree.path());

    assert!(
        search(&index, &options()).groups.is_empty(),
        "the same size and edges do not make two large files equal"
    );
}

fn endless_pair(folder: &str) -> (TempTree, Index) {
    let tree = TempTree::new().expect("temp tree is created");
    let directory = tree
        .root()
        .expect("root opens")
        .directory(folder)
        .expect("directory is created");
    for name in ["one", "two"] {
        directory.sparse(name, ENDLESS).expect("file is written");
    }
    let index = index_of(tree.path());
    (tree, index)
}

#[test]
fn a_search_stops_inside_a_file_it_is_hashing() {
    let (_tree, index) = endless_pair("big");
    let pool = HashingPool::new(None).expect("hashing pool starts");
    let options = options();
    let cancel = options.cancel.clone();
    let stopper = thread::spawn(move || {
        thread::sleep(STOP_AFTER);
        cancel.cancel();
    });
    let started = Instant::now();

    let outcome = find_duplicates(&index, &pool, &options);

    stopper.join().expect("the stopper finishes");
    assert!(matches!(outcome, Err(SearchError::Cancelled)), "{outcome:?}");
    let elapsed = started.elapsed();
    assert!(elapsed < STOPPED_WITHIN, "the search stopped after {elapsed:?}");
}

#[test]
fn the_cloud_time_limit_ends_a_file_being_hashed() {
    let (_tree, index) = endless_pair("cloud");
    let limited = DuplicateOptions {
        cloud_time_limit: STOP_AFTER,
        ..DuplicateOptions::new(CloudFolders::new(vec![index.path(node(&index, "cloud"))]))
    };
    let started = Instant::now();

    let duplicates = search(&index, &limited);

    let elapsed = started.elapsed();
    assert!(elapsed < STOPPED_WITHIN, "the cloud pass ended after {elapsed:?}");
    assert!(duplicates.groups.is_empty());
    assert!(
        duplicates
            .skipped
            .iter()
            .any(|skipped| skipped.reason == SkipReason::TimeLimit),
        "{:?}",
        duplicates.skipped
    );
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
