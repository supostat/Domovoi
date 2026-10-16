use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_apfs_sys::{container_capacity, real_dev, volume_capabilities, volume_used};
use domovoi_scanner::{NodeState, ScanOptions, Since, VolumeEvent, VolumeEventStream, history_store, scan};
use domovoi_testkit::{ImageVolume, Visibility, VolumeFormat, test_timeout};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

fn command_output(program: &str, arguments: &[&str], target: &Path) -> String {
    let output = Command::new(program)
        .args(arguments)
        .arg(target)
        .output()
        .expect("system tool runs");
    assert!(
        output.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("tool output is UTF-8")
}

fn plist_integer(plist: &str, key: &str) -> u64 {
    plist
        .split_once(&format!("<key>{key}</key>"))
        .and_then(|(_, rest)| rest.split_once("<integer>"))
        .and_then(|(_, rest)| rest.split_once("</integer>"))
        .and_then(|(value, _)| value.parse().ok())
        .unwrap_or_else(|| panic!("diskutil reports {key}"))
}

fn df_used_bytes(volume: &Path) -> u64 {
    let report = command_output("/bin/df", &["-k"], volume);
    let used: u64 = report
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(2))
        .and_then(|used| used.parse().ok())
        .expect("df reports the used column");
    used * 1024
}

fn wait_for_event(events: &Receiver<VolumeEvent>, accepts: impl Fn(&VolumeEvent) -> bool) -> VolumeEvent {
    let deadline = Instant::now() + test_timeout();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = events
            .recv_timeout(remaining)
            .expect("expected volume event arrives in time");
        if accepts(&event) {
            return event;
        }
    }
}

fn wait_for_history_store(mount_point: &Path) -> String {
    let deadline = Instant::now() + test_timeout();
    loop {
        if let Some(store) = history_store(mount_point).expect("device is readable") {
            return store;
        }
        assert!(
            Instant::now() < deadline,
            "browsable volume never got an event history store"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("path canonicalizes")
}

#[test]
fn case_sensitive_image_volume() {
    let mut volume =
        ImageVolume::create(VolumeFormat::CaseSensitiveApfs, Visibility::Hidden).expect("image is attached");
    let mount_point = canonical(volume.mount_point());

    mount_point_is_another_volume(&volume, &mount_point);
    names_differing_only_in_case_coexist(&mount_point);
    volume_attributes_match_the_system_tools(&mount_point);
    assert_eq!(history_store(&mount_point).expect("device is readable"), None);

    volume.detach().expect("hidden image detaches");
    volume
        .attach(Visibility::Browsable)
        .expect("image reattaches browsable");
    wait_for_history_store(&mount_point);
    realtime_events_and_unmount(&mut volume, &mount_point);
}

fn mount_point_is_another_volume(volume: &ImageVolume, mount_point: &Path) {
    let report = scan(volume.workspace(), &ScanOptions::default()).expect("workspace scans");
    let mount = report
        .root
        .children
        .iter()
        .find(|child| child.name == "mount")
        .expect("mount point is listed");
    assert_eq!(mount.state, NodeState::OtherVolume);
    assert!(mount.children.is_empty());
    assert_ne!(
        real_dev(mount_point).expect("mounted volume real dev"),
        real_dev(volume.workspace()).expect("workspace real dev")
    );
}

fn names_differing_only_in_case_coexist(mount_point: &Path) {
    assert!(volume_capabilities(mount_point).expect("capabilities").case_sensitive);
    fs::write(mount_point.join("Report"), b"upper").expect("upper-case file is written");
    fs::write(mount_point.join("report"), b"lower").expect("lower-case file is written");
    let report = scan(mount_point, &ScanOptions::default()).expect("mounted volume scans");
    let names: Vec<_> = report.root.children.iter().map(|child| child.name.clone()).collect();
    assert!(
        names.contains(&"Report".into()) && names.contains(&"report".into()),
        "{names:?}"
    );
}

fn volume_attributes_match_the_system_tools(mount_point: &Path) {
    let plist = command_output("/usr/sbin/diskutil", &["info", "-plist"], mount_point);
    let capacity = container_capacity(mount_point).expect("container capacity is read");
    assert_eq!(capacity.size, plist_integer(&plist, "APFSContainerSize"));
    assert_eq!(capacity.available, plist_integer(&plist, "APFSContainerFree"));
    assert_eq!(
        volume_used(mount_point).expect("used space is read"),
        df_used_bytes(mount_point)
    );
}

fn realtime_events_and_unmount(volume: &mut ImageVolume, mount_point: &Path) {
    let (sender, events) = channel();
    let stream = VolumeEventStream::watch(mount_point, Since::Now, sender).expect("event stream starts");
    fs::write(mount_point.join("fresh"), b"new").expect("file is written after the stream started");
    wait_for_event(
        &events,
        |event| matches!(event, VolumeEvent::Changed { path, .. } if path.starts_with(mount_point)),
    );
    volume.detach().expect("browsable image detaches");
    wait_for_event(&events, |event| matches!(event, VolumeEvent::Unmount { .. }));
    drop(stream);
}
