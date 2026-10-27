use std::path::{Path, PathBuf};

use domovoi_index::{TIME_MACHINE_PLIST, TimeMachineSettings};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn backups_switched_on_explain_purgeable_space() {
    let settings = TimeMachineSettings::read(&fixture("time-machine-on.plist"));
    assert_eq!(
        settings,
        TimeMachineSettings {
            auto_backup: Some(true),
            mobile_backups: Some(true),
        }
    );
    assert!(settings.explains_purgeable());
}

#[test]
fn a_missing_key_stays_unknown_and_backups_off_explain_nothing() {
    let settings = TimeMachineSettings::read(&fixture("time-machine-off.plist"));
    assert_eq!(
        settings,
        TimeMachineSettings {
            auto_backup: Some(false),
            mobile_backups: None,
        }
    );
    assert!(!settings.explains_purgeable());
}

#[test]
fn a_missing_file_leaves_every_setting_unknown() {
    let settings = TimeMachineSettings::read(&fixture("no-such-file.plist"));
    assert_eq!(settings, TimeMachineSettings::default());
    assert!(!settings.explains_purgeable());
    assert!(
        Path::new(TIME_MACHINE_PLIST).is_absolute(),
        "the live settings live at an absolute path"
    );
}
