use std::path::Path;

use domovoi_apfs_sys::data_volume;
use domovoi_index::{HiddenSpace, Measured, Source, Volumes};

const GIGABYTE: u64 = 1_000_000_000;

#[test]
fn hidden_space_is_the_volume_minus_what_the_scan_saw() {
    let purgeable = Measured {
        value: 12 * GIGABYTE,
        source: Source::MacOsEstimate,
    };
    let hidden = HiddenSpace::new(300 * GIGABYTE, 250 * GIGABYTE, purgeable, 4, Vec::new());
    assert_eq!(hidden.total.value, 50 * 1_000_000_000);
    assert_eq!(hidden.total.source, Source::Getattrlist);
    assert_eq!(hidden.overhead, 38 * 1_000_000_000);
    assert_eq!(hidden.hidden_folders, 4);
    assert!(!hidden.is_discrepancy());
    let overcounted = HiddenSpace::new(100 * GIGABYTE, 101 * GIGABYTE, purgeable, 0, Vec::new());
    assert!(overcounted.is_discrepancy());
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
