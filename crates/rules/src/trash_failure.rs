use crate::deny_reason::DenyReason;

const NO_PERMISSION: isize = 513;
const READ_ONLY_VOLUME: isize = 642;
const NO_SUCH_FILE: isize = 4;
const NO_TRASH_ON_VOLUME: isize = 3328;
const OUT_OF_SPACE: isize = 640;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrashFailure {
    Denied(DenyReason),
    NoPermission,
    ReadOnlyVolume,
    Missing,
    NoTrashOnVolume,
    OutOfSpace,
    Other { code: isize, description: String },
}

impl TrashFailure {
    pub(crate) fn from_cocoa(code: isize, description: String) -> Self {
        match code {
            NO_PERMISSION => Self::NoPermission,
            READ_ONLY_VOLUME => Self::ReadOnlyVolume,
            NO_SUCH_FILE => Self::Missing,
            NO_TRASH_ON_VOLUME => Self::NoTrashOnVolume,
            OUT_OF_SPACE => Self::OutOfSpace,
            code => Self::Other { code, description },
        }
    }
}
