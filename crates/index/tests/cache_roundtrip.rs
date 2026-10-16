mod synthetic_tree;

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, UNIX_EPOCH};

use domovoi_index::{CacheError, CacheHeader, Enrichment, Index, IndexRoot, open_cache, write_cache};
use domovoi_scanner::{Node, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use synthetic_tree::{data_volume, directory, file, opened_file};

const LAST_EVENT: u64 = 42;
const DIRECTORIES: usize = 1_000;
const FILES_PER_DIRECTORY: usize = 1_000;
const OPEN_BUDGET: Duration = Duration::from_secs(1);

fn header(app_version: &str) -> CacheHeader {
    CacheHeader {
        magic: *b"DOMOVOI\0",
        format: 1,
        volume_uuid: "A1B2C3D4-0000-0000-0000-000000000001".to_owned(),
        event_store: Some("EVENTS-1".to_owned()),
        os_build: "25B78".to_owned(),
        app_version: app_version.to_owned(),
    }
}

fn small_index() -> Index {
    let opened = UNIX_EPOCH + Duration::from_secs(1_750_000_000);
    let (report, root) = data_volume(vec![directory(
        "Users",
        vec![directory("igor", vec![file("a", 3), opened_file("b", 2, opened)])],
    )]);
    Index::build(&report, root, &Enrichment::default())
}

#[test]
fn a_written_cache_opens_into_the_same_index() {
    let tree = TempTree::new().expect("temp tree is created");
    let path = tree.path().join("scan.cache");
    let index = small_index();
    write_cache(&path, &header("1.0"), LAST_EVENT, &index).expect("cache is written");
    let leftovers: Vec<_> = fs::read_dir(tree.path())
        .expect("directory lists")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(leftovers, ["scan.cache"], "the write leaves no partial file behind");
    let cached = open_cache(&path, &header("1.0")).expect("cache opens");
    assert_eq!(cached.index, index);
    assert_eq!(cached.last_event_id, LAST_EVENT);
}

#[test]
fn a_cache_from_another_volume_system_or_version_is_refused() {
    let tree = TempTree::new().expect("temp tree is created");
    let path = tree.path().join("scan.cache");
    write_cache(&path, &header("1.0"), LAST_EVENT, &small_index()).expect("cache is written");
    assert!(matches!(open_cache(&path, &header("1.1")), Err(CacheError::Mismatch)));
    let other_volume = CacheHeader {
        volume_uuid: "OTHER".to_owned(),
        ..header("1.0")
    };
    assert!(matches!(open_cache(&path, &other_volume), Err(CacheError::Mismatch)));
}

#[test]
fn a_damaged_cache_is_refused() {
    let tree = TempTree::new().expect("temp tree is created");
    let path = tree.path().join("scan.cache");
    write_cache(&path, &header("1.0"), LAST_EVENT, &small_index()).expect("cache is written");
    let mut bytes = fs::read(&path).expect("cache reads");
    let length = bytes.len();
    bytes.truncate(length / 2);
    fs::write(&path, bytes).expect("damaged cache is written");
    assert!(matches!(open_cache(&path, &header("1.0")), Err(CacheError::Corrupt(_))));
}

#[test]
fn a_million_node_cache_opens_within_a_second() {
    let directories: Vec<Node> = (0..DIRECTORIES)
        .map(|index| {
            directory(
                &format!("project-{index}"),
                (0..FILES_PER_DIRECTORY)
                    .map(|number| file(&format!("file-{number}.o"), 1))
                    .collect(),
            )
        })
        .collect();
    let (report, root) = data_volume(vec![directory("Users", vec![directory("igor", directories)])]);
    let index = Index::build(&report, root, &Enrichment::default());
    assert!(index.len() > DIRECTORIES * FILES_PER_DIRECTORY);
    let tree = TempTree::new().expect("temp tree is created");
    let path = tree.path().join("large.cache");
    write_cache(&path, &header("1.0"), LAST_EVENT, &index).expect("cache is written");
    let started = Instant::now();
    let cached = open_cache(&path, &header("1.0")).expect("cache opens");
    let elapsed = started.elapsed();
    assert_eq!(cached.index.len(), index.len());
    assert!(elapsed <= OPEN_BUDGET, "opening took {elapsed:?}");
}

#[test]
fn a_file_system_delta_rescans_the_changed_subtree() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let project = root.directory("project").expect("directory is created");
    project.file("old", 8_192, Fill::Byte(1)).expect("file is written");
    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    let mut index = Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        &Enrichment::default(),
    );
    let before = index.node(index.root()).allocated;
    project
        .file("new", 40_960, Fill::Noise(5))
        .expect("file is written after the scan");
    let changed: PathBuf = fs::canonicalize(tree.path().join("project")).expect("path canonicalizes");
    index
        .rescan(&changed, &ScanOptions::default(), &Enrichment::default())
        .expect("subtree rescans");
    let project_node = index.find(&changed).expect("project is indexed");
    assert!(index.child_named(project_node, "new".as_ref()).is_some());
    assert!(index.node(index.root()).allocated > before);
    assert_eq!(index.node(index.root()).files, 2);
}
