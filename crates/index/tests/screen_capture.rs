use std::fs;
use std::path::{Path, PathBuf};

use domovoi_index::{
    Badges, CacheError, CacheHeader, ChangedPath, Enrichment, Index, IndexRoot, open_cache, query_screen_captures,
    write_cache,
};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const LAST_EVENT: u64 = 7;
const FORMAT_WITHOUT_SCREEN_CAPTURES: u32 = 3;

fn canonical(tree: &TempTree) -> PathBuf {
    fs::canonicalize(tree.path()).expect("the tree canonicalizes")
}

fn listing(paths: &[PathBuf]) -> Enrichment {
    Enrichment {
        screen_captures: paths.to_vec(),
        ..Enrichment::default()
    }
}

fn build(tree: &TempTree, enrichment: &Enrichment) -> Index {
    let report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        enrichment,
    )
}

fn apply(index: &mut Index, changed: &Path, enrichment: &Enrichment) {
    let request = index.delta_request(&[ChangedPath {
        path: changed.to_path_buf(),
        subtree: false,
    }]);
    let listings = request
        .targets()
        .iter()
        .map(|target| target.read(&ScanOptions::default()).expect("the target reads"))
        .collect();
    index
        .apply_delta(&request, listings, enrichment)
        .expect("the request is current");
}

fn is_screen_capture(index: &Index, path: &Path) -> bool {
    let node = index
        .find(path)
        .unwrap_or_else(|| panic!("{} is indexed", path.display()));
    index.node(node).badges.contains(Badges::SCREEN_CAPTURE)
}

fn header(format: u32) -> CacheHeader {
    CacheHeader {
        magic: *b"DOMOVOI\0",
        format,
        volume_uuid: "A1B2C3D4-0000-0000-0000-000000000001".to_owned(),
        event_store: Some("EVENTS-1".to_owned()),
        os_build: "25B78".to_owned(),
        app_version: "1.0".to_owned(),
    }
}

#[test]
fn only_a_listed_file_is_a_screen_capture() {
    let tree = TempTree::new().expect("temp tree is created");
    let desktop = tree
        .root()
        .expect("root opens")
        .directory("Desktop")
        .expect("directory is created");
    desktop
        .file("shot.png", 4_096, Fill::Noise(1))
        .expect("file is written");
    desktop
        .file("photo.png", 4_096, Fill::Noise(2))
        .expect("file is written");
    desktop.directory("Folder").expect("directory is created");
    let root = canonical(&tree);
    let index = build(
        &tree,
        &listing(&[root.join("Desktop/shot.png"), root.join("Desktop/Folder")]),
    );
    assert!(is_screen_capture(&index, &root.join("Desktop/shot.png")));
    assert!(!is_screen_capture(&index, &root.join("Desktop/photo.png")));
    assert!(!is_screen_capture(&index, &root.join("Desktop/Folder")));
}

#[test]
fn a_file_added_by_a_delta_is_a_screen_capture_when_listed() {
    let tree = TempTree::new().expect("temp tree is created");
    let desktop = tree
        .root()
        .expect("root opens")
        .directory("Desktop")
        .expect("directory is created");
    desktop.file("old.png", 4_096, Fill::Noise(3)).expect("file is written");
    let mut index = build(&tree, &Enrichment::default());
    desktop
        .file("fresh.png", 4_096, Fill::Noise(4))
        .expect("file is written after the scan");
    desktop
        .file("drawing.png", 4_096, Fill::Noise(5))
        .expect("file is written after the scan");
    let root = canonical(&tree);
    apply(
        &mut index,
        &root.join("Desktop"),
        &listing(&[root.join("Desktop/fresh.png")]),
    );
    assert!(is_screen_capture(&index, &root.join("Desktop/fresh.png")));
    assert!(!is_screen_capture(&index, &root.join("Desktop/drawing.png")));
}

#[test]
fn the_screen_capture_badge_survives_the_cache() {
    let tree = TempTree::new().expect("temp tree is created");
    let desktop = tree
        .root()
        .expect("root opens")
        .directory("Desktop")
        .expect("directory is created");
    desktop
        .file("shot.png", 4_096, Fill::Noise(6))
        .expect("file is written");
    let root = canonical(&tree);
    let index = build(&tree, &listing(&[root.join("Desktop/shot.png")]));
    let path = tree.path().join("scan.cache");
    let current = header(FORMAT_WITHOUT_SCREEN_CAPTURES + 1);
    write_cache(&path, &current, LAST_EVENT, &index).expect("cache is written");
    let cached = open_cache(&path, &current).expect("cache opens").index;
    assert_eq!(cached, index);
    assert!(is_screen_capture(&cached, &root.join("Desktop/shot.png")));
}

#[test]
fn a_cache_written_before_screen_captures_is_refused() {
    let tree = TempTree::new().expect("temp tree is created");
    let current = CacheHeader::current(tree.path(), "1.0").expect("the header describes the volume");
    assert_eq!(current.format, FORMAT_WITHOUT_SCREEN_CAPTURES + 1);
    let previous = CacheHeader {
        format: FORMAT_WITHOUT_SCREEN_CAPTURES,
        ..current.clone()
    };
    let path = tree.path().join("scan.cache");
    write_cache(&path, &previous, LAST_EVENT, &build(&tree, &Enrichment::default())).expect("cache is written");
    assert!(matches!(open_cache(&path, &current), Err(CacheError::Mismatch)));
}

#[test]
fn spotlight_answers_the_screen_capture_query() {
    let screen_captures = query_screen_captures().expect("the Spotlight query runs");
    assert!(screen_captures.iter().all(|path| path.is_absolute()));
}
