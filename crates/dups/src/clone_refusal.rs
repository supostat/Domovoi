use std::path::PathBuf;

use domovoi_rules::DenyReason;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloneRefusal {
    NotRegularFile(PathBuf),
    OtherVolume,
    VolumeCannotClone,
    VolumeCannotSwap,
    HardLinked,
    NotOwned,
    NotWritable(PathBuf),
    ProtectedFlags(u32),
    Purgeable,
    Busy,
    Denied(DenyReason),
    InsidePackage(PathBuf),
    SyncedFolder,
    OpenElsewhere(Vec<libc::pid_t>),
    AlreadyClones,
    ContentChanged,
}
