use std::io;

const NO_PERMISSION: isize = 513;
const READ_ONLY_VOLUME: isize = 642;
const NO_SUCH_FILE: isize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteFailure {
    Missing,
    Replaced,
    NoPermission,
    ReadOnlyVolume,
    Other { code: isize, description: String },
}

impl DeleteFailure {
    pub(crate) fn from_cocoa(code: isize, description: String) -> Self {
        match code {
            NO_PERMISSION => Self::NoPermission,
            READ_ONLY_VOLUME => Self::ReadOnlyVolume,
            NO_SUCH_FILE => Self::Missing,
            code => Self::Other { code, description },
        }
    }

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
}
