mod live_dump;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use domovoi_apfs_sys::{EntryAttributes, ObjectKind};
use domovoi_index::{
    ChangedPath, DeltaApplied, DeltaRequest, DirectoryListing, Enrichment, Index, IndexRoot, ListedEntry,
};
use domovoi_scanner::{Node, NodeState, ScanReport};
use domovoi_testkit::TempTree;

const UNITS: usize = 1_000;
const OBJECTS_PER_UNIT: usize = 500;
const SMALL_ENTRIES: usize = 100;
const OBJECT_SIZE: u64 = 4_096;
const ATTEMPTS: usize = 3;
const INDEX_BUILD_SHARE: u32 = 100;
const CREATED: u64 = 1_700_000_000;

static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(10_000);

fn attributes(kind: ObjectKind, size: u64) -> EntryAttributes {
    let file_id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let created = UNIX_EPOCH + Duration::from_secs(CREATED);
    let file = kind != ObjectKind::Directory;
    EntryAttributes {
        kind,
        flags: 0,
        file_id,
        created,
        modified: created,
        added: Some(created),
        mount_status: (!file).then_some(0),
        link_count: file.then_some(1),
        total_size: file.then_some(size),
        alloc_size: file.then_some(size),
        data_alloc_size: file.then_some(size),
        private_size: None,
        link_id: Some(file_id),
        real_dev: Some(1),
        clone_id: Some(file_id),
        ext_flags: Some(0),
        clone_refcount: file.then_some(1),
    }
}

fn file(name: &str) -> Node {
    Node {
        name: OsString::from(name),
        attributes: Some(attributes(ObjectKind::File, OBJECT_SIZE)),
        last_used: None,
        state: NodeState::Leaf,
        children: Vec::new(),
    }
}

fn directory(name: &str, children: Vec<Node>) -> Node {
    Node {
        name: OsString::from(name),
        attributes: Some(attributes(ObjectKind::Directory, 0)),
        last_used: None,
        state: NodeState::Listed,
        children,
    }
}

fn bulk(units: usize, objects: usize) -> Node {
    directory(
        "bulk",
        (0..units)
            .map(|unit| {
                directory(
                    &format!("unit-{unit}"),
                    (0..objects).map(|object| file(&format!("object-{object}.o"))).collect(),
                )
            })
            .collect(),
    )
}

fn small() -> Node {
    directory(
        "small",
        (0..SMALL_ENTRIES)
            .map(|entry| file(&format!("entry-{entry}")))
            .collect(),
    )
}

fn shallow(node: &Node) -> Node {
    Node {
        children: Vec::new(),
        ..node.clone()
    }
}

fn report(root: &Path, children: Vec<Node>) -> ScanReport {
    ScanReport {
        root: Node {
            name: root.as_os_str().to_owned(),
            attributes: None,
            last_used: None,
            state: NodeState::Listed,
            children,
        },
        files: 0,
        bytes: 0,
    }
}

fn disk_root() -> (TempTree, PathBuf) {
    let tree = TempTree::new().expect("temp tree is created");
    for folder in ["bulk", "small"] {
        fs::create_dir(tree.path().join(folder)).expect("the folder exists on disk for its delta target");
    }
    let root = fs::canonicalize(tree.path()).expect("the root resolves");
    (tree, root)
}

fn build(report: &ScanReport, root: &Path) -> Index {
    Index::build(
        report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn listing(index: &Index, path: PathBuf, entries: Vec<ListedEntry>) -> (DeltaRequest, DirectoryListing) {
    let request = index.delta_request(&[ChangedPath { path, subtree: false }]);
    let [target] = request.targets() else {
        panic!("one target is expected: {request:?}");
    };
    let listing = target.listing(entries, 0);
    (request, listing)
}

fn fastest(mut attempt: impl FnMut() -> Duration) -> Duration {
    (0..ATTEMPTS)
        .map(|_| attempt())
        .min()
        .expect("at least one attempt runs")
}

#[test]
fn a_directory_delta_costs_at_most_a_hundredth_of_an_index_build() {
    let (_tree, root) = disk_root();
    let (bulk, small, readme) = (bulk(UNITS, OBJECTS_PER_UNIT), small(), file("readme"));
    let report = report(&root, vec![bulk.clone(), small.clone(), readme.clone()]);
    let index_build = fastest(|| {
        let started = Instant::now();
        drop(build(&report, &root));
        started.elapsed()
    });
    let index = build(&report, &root);

    let (root_request, root_listing) = listing(
        &index,
        index.path(index.root()),
        vec![
            ListedEntry::Kept(shallow(&bulk)),
            ListedEntry::Kept(shallow(&small)),
            ListedEntry::Read(readme),
            ListedEntry::Read(file("fresh")),
        ],
    );
    let (small_request, small_listing) = listing(
        &index,
        index.path(index.root()).join("small"),
        (0..SMALL_ENTRIES)
            .map(|entry| ListedEntry::Read(file(&format!("entry-{entry}"))))
            .collect(),
    );
    for (name, request, listing, added) in [
        ("root", root_request, root_listing, 1),
        ("small", small_request, small_listing, 0),
    ] {
        let delta = fastest(|| {
            let mut copy = index.clone();
            let listings = vec![listing.clone()];
            let started = Instant::now();
            copy.apply_delta(&request, listings, &Enrichment::default())
                .expect("the request is current");
            let elapsed = started.elapsed();
            assert_eq!(copy.len(), index.len() + added, "{name}");
            assert_eq!(
                copy.node(copy.root()).files,
                index.node(index.root()).files + u32::try_from(added).expect("one file"),
                "{name}"
            );
            elapsed
        });
        assert!(
            delta * INDEX_BUILD_SHARE <= index_build,
            "the {name} delta took {delta:?}, the index build {index_build:?}"
        );
    }
}

#[test]
fn compaction_past_the_threshold_matches_a_fresh_build() {
    let (_tree, root) = disk_root();
    let (small, readme) = (small(), file("readme"));
    let before = report(&root, vec![bulk(20, 100), small.clone(), readme.clone()]);
    let mut index = build(&before, &root);
    let replacement = bulk(5, 100);
    let (request, listing) = listing(
        &index,
        index.path(index.root()),
        vec![
            ListedEntry::Read(replacement.clone()),
            ListedEntry::Kept(shallow(&small)),
            ListedEntry::Read(readme.clone()),
        ],
    );
    let applied = index
        .apply_delta(&request, vec![listing], &Enrichment::default())
        .expect("the request is current");
    assert_eq!(
        applied,
        DeltaApplied::Compacted {
            directories: vec![index.root()]
        },
        "the replaced folder leaves more garbage than live nodes"
    );
    let fresh = build(&report(&root, vec![replacement, small, readme]), &root);
    assert_eq!(live_dump::dump(&index), live_dump::dump(&fresh));
    assert_eq!(index.len(), fresh.len());
}
