use std::path::Path;
use std::time::{Duration, Instant};

use domovoi_scanner::{STALL_LIMIT, ScanError, ScanOptions, StallMonitor, scan};
use domovoi_testkit::{Fill, TempTree};

const SECOND: Duration = Duration::from_secs(1);

#[test]
fn the_monitor_reports_a_stall_only_after_the_limit_of_silence() {
    let started = Instant::now();
    let monitor = StallMonitor::starting_at(started);
    monitor.enter(Path::new("Library/Caches"), started);
    assert!(!monitor.is_stalled(started + 59 * SECOND, STALL_LIMIT));
    assert!(monitor.is_stalled(started + STALL_LIMIT + SECOND, STALL_LIMIT));
    assert_eq!(monitor.last_path(), Path::new("Library/Caches"));

    monitor.heartbeat(started + 50 * SECOND);
    assert!(
        !monitor.is_stalled(started + 100 * SECOND, STALL_LIMIT),
        "activity at 50 s leaves 50 s of silence at 100 s"
    );
    assert_eq!(monitor.idle_for(started + 100 * SECOND), 50 * SECOND);
    assert!(monitor.is_stalled(started + 111 * SECOND, STALL_LIMIT));

    monitor.enter(Path::new("Users/igor"), started + 120 * SECOND);
    assert!(!monitor.is_stalled(started + 150 * SECOND, STALL_LIMIT));
    assert_eq!(monitor.last_path(), Path::new("Users/igor"));
    assert!(
        StallMonitor::default().idle_for(Instant::now()) < SECOND,
        "a fresh monitor counts from its creation"
    );
}

#[test]
fn a_responsive_tree_scans_to_the_end_under_the_monitor() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let notes = root.directory("notes").expect("directory is created");
    for number in 0..20 {
        notes
            .file(format!("note-{number}.txt"), 512, Fill::Byte(number as u8))
            .expect("file is written");
    }
    root.file("photo.heic", 4_096, Fill::Noise(7)).expect("file is written");

    let report = scan(tree.path(), &ScanOptions::default()).expect("a responsive tree scans to the end");

    assert_eq!(report.files, 21);
    assert!(report.bytes > 0);
}

#[test]
fn an_unresponsive_scan_names_the_folder_it_waited_on() {
    let error = ScanError::Unresponsive {
        path: Path::new("/Volumes/Backup/Photos").to_path_buf(),
    };
    let text = error.to_string();
    assert!(text.contains("/Volumes/Backup/Photos"), "{text}");
    assert!(text.contains("stopped responding"), "{text}");
}
