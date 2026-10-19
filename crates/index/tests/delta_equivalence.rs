mod live_dump;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_index::{Category, ChangedPath, Enrichment, Index, IndexRoot, StaleDelta};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const UNREADABLE: libc::mode_t = 0o000;
const SHARED_HOME: &str = "/Users/Shared";

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn change(index: &Index, relative: &str, subtree: bool) -> ChangedPath {
    let root = index.path(index.root());
    ChangedPath {
        path: if relative.is_empty() { root } else { root.join(relative) },
        subtree,
    }
}

fn apply(index: &mut Index, changed: &[(&str, bool)]) {
    let changed: Vec<ChangedPath> = changed
        .iter()
        .map(|(relative, subtree)| change(index, relative, *subtree))
        .collect();
    let request = index.delta_request(&changed);
    let listings = request
        .targets()
        .iter()
        .map(|target| target.read(&ScanOptions::default()).expect("the target reads"))
        .collect();
    let applied = index
        .apply_delta(&request, listings, &Enrichment::default())
        .expect("the request is current");
    let directories = applied.directories();
    assert!(directories.iter().all(|directory| index.is_live(*directory)));
    let mut reported: Vec<PathBuf> = directories.iter().map(|directory| index.path(*directory)).collect();
    let mut targets: Vec<PathBuf> = request
        .targets()
        .iter()
        .map(|target| target.path().to_path_buf())
        .collect();
    reported.sort();
    targets.sort();
    assert_eq!(reported, targets);
}

fn assert_matches_a_fresh_build(index: &Index, root: &Path) {
    let fresh = index_of(root);
    let (delta, full) = (live_dump::dump(index), live_dump::dump(&fresh));
    for (path, expected) in &full {
        assert_eq!(delta.get(path), Some(expected), "{} after the delta", path.display());
    }
    assert_eq!(delta.len(), full.len(), "the delta keeps no extra nodes");
    assert_eq!(index.len(), fresh.len());
    assert_eq!(index.hidden_folders(), fresh.hidden_folders());
}

fn under(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

fn clone_file(source: &Path, target: &Path) {
    let status = Command::new("/bin/cp")
        .arg("-c")
        .arg(source)
        .arg(target)
        .status()
        .expect("cp runs");
    assert!(status.success(), "cp -c failed");
}

fn documents() -> TempTree {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let docs = root.directory("docs").expect("directory is created");
    docs.file("a", 8_192, Fill::Byte(1)).expect("file is written");
    docs.file("b", 4_096, Fill::Byte(2)).expect("file is written");
    let old = docs.directory("old").expect("directory is created");
    old.file("kept", 16_384, Fill::Noise(3)).expect("file is written");
    old.directory("inner")
        .expect("directory is created")
        .file("deep", 4_096, Fill::Byte(4))
        .expect("file is written");
    root.file("readme", 4_096, Fill::Byte(5)).expect("file is written");
    tree
}

#[test]
fn files_appear_change_and_vanish() {
    let tree = documents();
    let mut index = index_of(tree.path());
    fs::write(under(tree.path(), "docs/c"), vec![7_u8; 20_000]).expect("a file is added");
    fs::write(under(tree.path(), "docs/a"), vec![8_u8; 40_000]).expect("a file grows");
    fs::remove_file(under(tree.path(), "docs/b")).expect("a file is removed");
    apply(&mut index, &[("docs", false)]);
    assert_matches_a_fresh_build(&index, tree.path());
}

#[test]
fn a_new_folder_is_read_whole_and_a_removed_one_drops_its_subtree() {
    let tree = documents();
    let mut index = index_of(tree.path());
    fs::create_dir_all(under(tree.path(), "docs/new/inner")).expect("folders are added");
    fs::write(under(tree.path(), "docs/new/inner/x"), vec![1_u8; 12_000]).expect("a file is added");
    apply(&mut index, &[("docs/new/inner", false)]);
    assert_matches_a_fresh_build(&index, tree.path());
    fs::remove_dir_all(under(tree.path(), "docs/old")).expect("a folder is removed");
    apply(&mut index, &[("docs/old", false)]);
    assert_matches_a_fresh_build(&index, tree.path());
}

#[test]
fn nested_targets_of_one_batch_report_their_live_directories() {
    let tree = documents();
    let mut index = index_of(tree.path());
    fs::write(under(tree.path(), "docs/d"), vec![4_u8; 6_000]).expect("a file is added");
    fs::write(under(tree.path(), "docs/old/inner/e"), vec![5_u8; 7_000]).expect("a deep file is added");
    fs::write(under(tree.path(), "docs/old/f"), vec![6_u8; 8_000]).expect("a file is added");
    apply(
        &mut index,
        &[("docs", false), ("docs/old", false), ("docs/old/inner", false)],
    );
    assert_matches_a_fresh_build(&index, tree.path());
}

#[test]
fn a_replaced_folder_is_read_again() {
    let tree = documents();
    let mut index = index_of(tree.path());
    fs::remove_dir_all(under(tree.path(), "docs/old")).expect("a folder is removed");
    fs::create_dir_all(under(tree.path(), "docs/old/other")).expect("the folder is created again");
    fs::write(under(tree.path(), "docs/old/other/y"), vec![2_u8; 30_000]).expect("a file is added");
    apply(&mut index, &[("docs", false)]);
    assert_matches_a_fresh_build(&index, tree.path());
}

#[test]
fn a_project_marker_changes_the_category_of_its_folder() {
    let tree = TempTree::under(Path::new(SHARED_HOME)).expect("a tree is created under a home folder");
    let root = tree.root().expect("root opens");
    let work = root.directory("work").expect("directory is created");
    work.file("notes.txt", 4_096, Fill::Byte(1)).expect("file is written");
    work.directory("src")
        .expect("directory is created")
        .file("main.rs", 4_096, Fill::Byte(2))
        .expect("file is written");
    let mut index = index_of(tree.path());
    let category_of_work = |index: &Index| {
        let work = index.find(&change(index, "work", false).path).expect("work is indexed");
        index.node(work).category
    };
    assert_eq!(category_of_work(&index), Category::Home);
    fs::write(under(tree.path(), "work/Cargo.toml"), b"[package]\n").expect("a marker appears");
    apply(&mut index, &[("work", false)]);
    assert_eq!(category_of_work(&index), Category::Dev);
    assert_matches_a_fresh_build(&index, tree.path());
    fs::remove_file(under(tree.path(), "work/Cargo.toml")).expect("the marker is removed");
    apply(&mut index, &[("work", false)]);
    assert_eq!(category_of_work(&index), Category::Home);
    assert_matches_a_fresh_build(&index, tree.path());
}

#[test]
fn clones_and_hard_links_keep_their_canonical_copy() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.directory("media")
        .expect("directory is created")
        .file("a", 64_000, Fill::Noise(6))
        .expect("file is written");
    root.directory("other").expect("directory is created");
    let mut index = index_of(tree.path());
    clone_file(&under(tree.path(), "media/a"), &under(tree.path(), "media/b"));
    apply(&mut index, &[("media", false)]);
    assert_matches_a_fresh_build(&index, tree.path());
    fs::hard_link(under(tree.path(), "media/a"), under(tree.path(), "other/c")).expect("a hard link is made");
    apply(&mut index, &[("media", false), ("other", false)]);
    assert_matches_a_fresh_build(&index, tree.path());
    fs::remove_file(under(tree.path(), "media/a")).expect("the original is removed");
    apply(&mut index, &[("media", false), ("other", false)]);
    assert_matches_a_fresh_build(&index, tree.path());
}

#[test]
fn a_subtree_target_is_read_whole_and_unreadable_folders_stay_hidden() {
    let tree = documents();
    let mut index = index_of(tree.path());
    fs::write(under(tree.path(), "docs/old/inner/deeper"), vec![3_u8; 9_000]).expect("a deep file is added");
    let docs = tree.root().expect("root opens");
    fs::create_dir(under(tree.path(), "docs/locked")).expect("a folder is added");
    docs.mode("docs/locked", UNREADABLE).expect("the folder is closed");
    apply(&mut index, &[("docs", true)]);
    assert_matches_a_fresh_build(&index, tree.path());
    assert!(index.hidden_folders() > 0);
}

#[test]
fn a_request_made_before_another_change_is_refused() {
    let tree = documents();
    let mut index = index_of(tree.path());
    let early = index.delta_request(&[change(&index, "docs", false)]);
    let listings: Vec<_> = early
        .targets()
        .iter()
        .map(|target| target.read(&ScanOptions::default()).expect("the target reads"))
        .collect();
    fs::write(under(tree.path(), "readme"), vec![9_u8; 10_000]).expect("the root changes");
    apply(&mut index, &[("", false)]);
    assert_eq!(
        index.apply_delta(&early, listings, &Enrichment::default()),
        Err(StaleDelta)
    );
    assert_matches_a_fresh_build(&index, tree.path());
}
