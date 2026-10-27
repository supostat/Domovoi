use std::fmt::Write;

use domovoi_apfs_sys::{Snapshot, SnapshotClass, time_machine_stamp};

const DELETE_LOCAL_SNAPSHOTS: &str = "tmutil deletelocalsnapshots";
const THIN_LOCAL_SNAPSHOTS: &str = "tmutil thinlocalsnapshots / 10000000000 4";

pub fn tmutil_commands(snapshots: &[Snapshot]) -> String {
    let mut commands = String::new();
    for stamp in snapshots
        .iter()
        .filter_map(|snapshot| time_machine_stamp(&snapshot.name))
    {
        writeln!(commands, "{DELETE_LOCAL_SNAPSHOTS} {stamp}").expect("writing into a string never fails");
    }
    if snapshots
        .iter()
        .any(|snapshot| snapshot.class != SnapshotClass::TimeMachine)
    {
        commands.push_str(THIN_LOCAL_SNAPSHOTS);
        commands.push('\n');
    }
    commands
}
