use std::path::{Path, PathBuf};
use std::time::Duration;

use domovoi_apfs_sys::data_volume;
use domovoi_index::{SpaceRecovery, Volumes};
use domovoi_testkit::{ImageVolume, Visibility, VolumeFormat};

const TOLERANCE_SHARE: f64 = 0.05;
const TOLERANCE_FLOOR: u64 = 256 * 1024 * 1024;
const COMPLETION_WAIT: Duration = Duration::from_secs(10);

fn recovery() -> SpaceRecovery {
    SpaceRecovery::load().expect("this macOS offers the disk-space recovery functions")
}

fn image_mount_point(volume: &ImageVolume) -> PathBuf {
    std::fs::canonicalize(volume.mount_point()).expect("the mount point resolves")
}

#[test]
fn the_recovery_functions_exist_in_this_macos() {
    assert!(SpaceRecovery::load().is_some());
}

#[test]
fn the_data_volume_estimate_agrees_with_the_purgeable_measurement() {
    let data = data_volume().expect("data volume is mounted");
    let estimate = recovery()
        .estimate(&data)
        .expect("the mechanism estimates the data volume");
    let purgeable = Volumes::measure(&data)
        .expect("volume numbers are read")
        .purgeable
        .value;
    let tolerance = (estimate.max(purgeable) as f64 * TOLERANCE_SHARE) as u64 + TOLERANCE_FLOOR;
    assert!(
        estimate.abs_diff(purgeable) <= tolerance,
        "estimate {estimate} and purgeable {purgeable} differ by more than {tolerance}"
    );
}

#[test]
fn an_image_volume_has_nothing_to_recover() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let estimate = recovery()
        .estimate(&image_mount_point(&volume))
        .expect("the mechanism estimates the image");
    assert_eq!(estimate, 0);
}

#[test]
fn a_request_for_nothing_completes_at_once_and_cancels_quietly() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let recovery = recovery();
    let mut handle = recovery
        .start(&image_mount_point(&volume), 0)
        .expect("the mechanism takes the request");
    let outcome = handle.wait(COMPLETION_WAIT).expect("the completion arrives").clone();
    assert!(outcome.succeeded, "nothing was needed, so the request succeeds");
    assert_eq!(outcome.error, None);
    assert!(outcome.available > 0, "the completion reports the available space");
    assert!(!handle.is_running());
    recovery.cancel(&handle);
}

#[test]
fn a_path_outside_any_volume_is_refused_before_the_mechanism_runs() {
    assert!(recovery().estimate(Path::new("")).is_err());
}
