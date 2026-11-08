use std::io;

use domovoi_apfs_sys::AccessError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteFailure {
    Missing,
    Replaced,
    NoPermission,
    ReadOnlyVolume,
    DiffersFromKept,
    KeptCopyUnavailable,
    Other { code: isize, description: String },
}

impl DeleteFailure {
    pub(crate) fn from_io(error: &io::Error) -> Self {
        match error.raw_os_error() {
            Some(libc::ENOENT) => Self::Missing,
            Some(libc::EACCES | libc::EPERM) => Self::NoPermission,
            Some(libc::EROFS) => Self::ReadOnlyVolume,
            code => Self::Other {
                code: code.map_or(0, |code| isize::try_from(code).expect("an errno fits in isize")),
                description: error.to_string(),
            },
        }
    }

    pub(crate) fn from_access(error: &AccessError) -> Self {
        match error {
            AccessError::Tcc | AccessError::Unix => Self::NoPermission,
            AccessError::Missing => Self::Missing,
            AccessError::Other(error) => Self::from_io(error),
        }
    }
}
