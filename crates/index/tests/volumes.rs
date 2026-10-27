use std::path::Path;

use domovoi_apfs_sys::data_volume;
use domovoi_index::{HiddenSpace, Measured, Source, TimeMachineSettings, Volumes};
use domovoi_testkit::{ImageVolume, Visibility, VolumeFormat};

const GIGABYTE: u64 = 1_000_000_000;

#[test]
fn hidden_space_is_the_volume_minus_what_the_scan_saw() {
    let purgeable = Measured {
        value: 12 * GIGABYTE,
        source: Source::MacOsEstimate,
    };
    let backups = TimeMachineSettings {
        auto_backup: Some(true),
        mobile_backups: None,
    };
    let hidden = HiddenSpace::new(300 * GIGABYTE, 250 * GIGABYTE, purgeable, 4, Vec::new(), 2, backups);
    assert_eq!(hidden.total.value, 50 * 1_000_000_000);
    assert_eq!(hidden.total.source, Source::Getattrlist);
    assert_eq!(hidden.unaccounted, 38 * 1_000_000_000, "the remainder after purgeable");
    assert_eq!(hidden.hidden_folders, 4);
    assert_eq!(
        hidden.other_user_homes, 2,
        "the homes of other users are counted, not sized"
    );
    assert_eq!(hidden.time_machine, backups);
    assert!(hidden.time_machine.explains_purgeable());
    assert!(!hidden.is_discrepancy());
    let overcounted = HiddenSpace::new(
        100 * GIGABYTE,
        101 * GIGABYTE,
        purgeable,
        0,
        Vec::new(),
        0,
        TimeMachineSettings::default(),
    );
    assert!(overcounted.is_discrepancy());
    assert!(!overcounted.time_machine.explains_purgeable());
}

#[test]
fn every_volume_number_names_its_source() {
    let data = data_volume().expect("data volume is mounted");
    let volumes = Volumes::measure(&data).expect("volume numbers are read");
    for measured in [
        volumes.data_used,
        volumes.system_used,
        volumes.other_volumes,
        volumes.container_size,
        volumes.free,
    ] {
        assert_eq!(measured.source, Source::Getattrlist);
    }
    assert_eq!(volumes.purgeable.source, Source::MacOsEstimate);
    assert!(volumes.data_used.value > 0 && volumes.system_used.value > 0);
    assert!(
        volumes.other_volumes.value > 0,
        "VM and Preboot live in the same container"
    );
    assert!(volumes.container_size.value > volumes.free.value);
    assert!(Volumes::measure(Path::new("/nonexistent")).is_err());
}

#[test]
fn a_volume_of_another_container_counts_neither_macos_nor_its_neighbours() {
    let volume = ImageVolume::create(VolumeFormat::Apfs, Visibility::Hidden).expect("the image attaches");
    let mount_point = std::fs::canonicalize(volume.mount_point()).expect("the mount point resolves");
    let volumes = Volumes::measure(&mount_point).expect("volume numbers are read");
    assert_eq!(volumes.system_used.value, 0);
    assert_eq!(volumes.other_volumes.value, 0);
    assert!(volumes.data_used.value > 0);
}
