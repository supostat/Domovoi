use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use domovoi_scanner::{ScanOptions, ScanProgress, scan};
use domovoi_testkit::{Fill, TempTree};

const BACKLOG: usize = 4_096;
const TOLERANCE: f32 = 1e-4;
const NESTED_DIRECTORIES: u64 = 4;

fn nested_tree() -> TempTree {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let projects = root.directory("Projects").expect("directory is created");
    projects.file("README.md", 256, Fill::Byte(1)).expect("file is written");
    let engine = projects.directory("engine").expect("directory is created");
    engine.file("main.rs", 1_024, Fill::Byte(2)).expect("file is written");
    let site = projects.directory("site").expect("directory is created");
    site.file("index.html", 512, Fill::Byte(3)).expect("file is written");
    root.file("notes.txt", 128, Fill::Byte(4)).expect("file is written");
    tree
}

fn flat_tree() -> TempTree {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    for number in 0..5 {
        root.file(format!("file-{number}.bin"), 256, Fill::Byte(number as u8))
            .expect("file is written");
    }
    tree
}

fn shares(root: &Path, expected_directories: Option<u64>) -> (Vec<f32>, Option<f32>) {
    let (sender, receiver) = mpsc::sync_channel(BACKLOG);
    let listener = thread::spawn(move || receiver.iter().collect::<Vec<ScanProgress>>());
    let options = ScanOptions {
        workers: NonZeroUsize::new(1),
        progress: Some(sender),
        expected_directories,
        ..ScanOptions::default()
    };
    scan(root, &options).expect("the tree scans");
    drop(options);
    let events = listener.join().expect("the listener finishes");
    let walking: Vec<f32> = events
        .iter()
        .filter_map(|event| match event {
            ScanProgress::Walking { share, .. } => Some(*share),
            ScanProgress::Walked { .. } => None,
        })
        .collect();
    let walked = events.iter().find_map(|event| match event {
        ScanProgress::Walked { share, .. } => Some(*share),
        ScanProgress::Walking { .. } => None,
    });
    (walking, walked)
}

#[test]
fn the_relative_share_grows_with_every_folder_and_ends_at_one() {
    let tree = nested_tree();
    let (walking, walked) = shares(tree.path(), None);
    assert_eq!(walking.len() as u64, NESTED_DIRECTORIES, "one event per listed folder");
    for pair in walking.windows(2) {
        assert!(
            pair[0] <= pair[1] + TOLERANCE,
            "the share never falls back: {walking:?}"
        );
    }
    assert!(
        (walking.last().copied().expect("the walk reports") - 1.0).abs() < TOLERANCE,
        "the last folder completes the whole: {walking:?}"
    );
    assert!((walked.expect("the walk ends") - 1.0).abs() < TOLERANCE);
}

#[test]
fn a_tree_of_plain_files_completes_with_its_first_event() {
    let tree = flat_tree();
    let (walking, _) = shares(tree.path(), None);
    assert_eq!(walking.len(), 1);
    assert!((walking[0] - 1.0).abs() < TOLERANCE, "{walking:?}");
}

#[test]
fn an_expected_folder_count_turns_the_share_into_an_absolute_scale() {
    let tree = nested_tree();
    let (halved, _) = shares(tree.path(), Some(2 * NESTED_DIRECTORIES));
    assert!(
        (halved.last().copied().expect("the walk reports") - 0.5).abs() < TOLERANCE,
        "twice the folders expected leaves the scale at a half: {halved:?}"
    );
    let (clamped, walked) = shares(tree.path(), Some(NESTED_DIRECTORIES / 2));
    assert!(
        (clamped.last().copied().expect("the walk reports") - 1.0).abs() < TOLERANCE,
        "more folders than expected clamp at one: {clamped:?}"
    );
    assert!((walked.expect("the walk ends") - 1.0).abs() < TOLERANCE);
}
