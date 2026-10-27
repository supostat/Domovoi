const TIME_MACHINE_PREFIX: &str = "com.apple.TimeMachine.";
const TIME_MACHINE_SUFFIX: &str = ".local";
const OS_UPDATE_PREFIX: &str = "com.apple.os.update-";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotClass {
    TimeMachine,
    OsUpdate,
    ThirdParty,
}

impl SnapshotClass {
    pub fn of(name: &str) -> Self {
        if time_machine_stamp(name).is_some() {
            Self::TimeMachine
        } else if name.starts_with(OS_UPDATE_PREFIX) {
            Self::OsUpdate
        } else {
            Self::ThirdParty
        }
    }
}

pub fn time_machine_stamp(name: &str) -> Option<&str> {
    name.strip_prefix(TIME_MACHINE_PREFIX)?
        .strip_suffix(TIME_MACHINE_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::{SnapshotClass, time_machine_stamp};

    #[test]
    fn classifies_by_name_prefix() {
        assert_eq!(
            SnapshotClass::of("com.apple.TimeMachine.2026-10-16-120000.local"),
            SnapshotClass::TimeMachine
        );
        assert_eq!(SnapshotClass::of("com.apple.os.update-0A1B2C"), SnapshotClass::OsUpdate);
        assert_eq!(SnapshotClass::of("com.carbonclone.snapshot"), SnapshotClass::ThirdParty);
    }

    #[test]
    fn the_stamp_is_the_name_between_the_time_machine_affixes() {
        assert_eq!(
            time_machine_stamp("com.apple.TimeMachine.2026-10-16-120000.local"),
            Some("2026-10-16-120000")
        );
        assert_eq!(time_machine_stamp("com.apple.os.update-0A1B2C"), None);
    }
}
