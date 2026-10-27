use std::time::SystemTime;

use domovoi_apfs_sys::{Snapshot, SnapshotClass};
use domovoi_app::tmutil_commands;

fn snapshot(name: &str) -> Snapshot {
    Snapshot {
        name: name.to_owned(),
        created: SystemTime::UNIX_EPOCH,
        class: SnapshotClass::of(name),
    }
}

#[test]
fn every_time_machine_snapshot_gets_a_delete_line_and_the_rest_one_thin_line() {
    let snapshots = [
        snapshot("com.apple.TimeMachine.2026-10-16-120000.local"),
        snapshot("com.apple.os.update-0A1B2C"),
        snapshot("com.apple.TimeMachine.2026-10-17-090000.local"),
    ];
    let commands = tmutil_commands(&snapshots);
    let lines: Vec<&str> = commands.lines().collect();
    assert_eq!(
        lines,
        [
            "tmutil deletelocalsnapshots 2026-10-16-120000",
            "tmutil deletelocalsnapshots 2026-10-17-090000",
            "tmutil thinlocalsnapshots / 10000000000 4",
        ]
    );
    assert!(commands.ends_with('\n'), "the text pastes as whole commands");
    assert!(!commands.contains('!'));
}

#[test]
fn only_time_machine_snapshots_need_no_thinning() {
    let snapshots = [snapshot("com.apple.TimeMachine.2026-10-16-120000.local")];
    assert_eq!(
        tmutil_commands(&snapshots),
        "tmutil deletelocalsnapshots 2026-10-16-120000\n"
    );
}

#[test]
fn no_snapshots_give_an_empty_text() {
    assert_eq!(tmutil_commands(&[]), "");
}
