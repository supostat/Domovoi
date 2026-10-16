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
        if name.starts_with(TIME_MACHINE_PREFIX) && name.ends_with(TIME_MACHINE_SUFFIX) {
            Self::TimeMachine
        } else if name.starts_with(OS_UPDATE_PREFIX) {
            Self::OsUpdate
        } else {
            Self::ThirdParty
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SnapshotClass;

    #[test]
    fn classifies_by_name_prefix() {
        assert_eq!(
            SnapshotClass::of("com.apple.TimeMachine.2026-10-16-120000.local"),
            SnapshotClass::TimeMachine
        );
        assert_eq!(SnapshotClass::of("com.apple.os.update-0A1B2C"), SnapshotClass::OsUpdate);
        assert_eq!(SnapshotClass::of("com.carbonclone.snapshot"), SnapshotClass::ThirdParty);
    }
}
