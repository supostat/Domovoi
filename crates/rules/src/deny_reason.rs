use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenyReason {
    ProtectedFlag(&'static str),
    SystemVolume,
    OtherVolume,
    Firmlink,
    ParentNotWritable,
    ProtectedData(&'static str),
    RunningApplication(PathBuf),
    OwnApplication,
    Trash,
}
