use std::io;

use domovoi_apfs_sys::sys::{DIR_MNTSTATUS_MNTPOINT, DIR_MNTSTATUS_TRIGGER, SF_DATALESS, SF_FIRMLINK};
use domovoi_apfs_sys::{AccessError, EntryAttributes, ObjectKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    Leaf,
    Listed,
    OtherVolume,
    Firmlink,
    HiddenByPrivacy,
    DataVault,
    OtherUser,
    InCloud,
    Deferred,
    Vanished,
    Unreadable(i32),
}

impl NodeState {
    pub(crate) fn of_listed_entry(attributes: &EntryAttributes) -> Self {
        if attributes.flags & SF_DATALESS != 0 {
            Self::InCloud
        } else if attributes.kind != ObjectKind::Directory {
            Self::Leaf
        } else if attributes
            .mount_status
            .is_some_and(|status| status & (DIR_MNTSTATUS_MNTPOINT | DIR_MNTSTATUS_TRIGGER) != 0)
        {
            Self::OtherVolume
        } else if attributes.flags & SF_FIRMLINK != 0 {
            Self::Firmlink
        } else {
            Self::Listed
        }
    }

    pub(crate) fn of_entry_error(error: &AccessError) -> Self {
        match error {
            AccessError::Tcc => Self::HiddenByPrivacy,
            AccessError::Unix => Self::OtherUser,
            AccessError::Missing => Self::Vanished,
            AccessError::Other(error) => Self::of_errno(error),
        }
    }

    pub(crate) fn of_listing_error(error: &io::Error) -> Self {
        match error.raw_os_error() {
            Some(libc::EBADF | libc::EACCES) => Self::OtherUser,
            Some(libc::EPERM) => Self::HiddenByPrivacy,
            _ => Self::of_errno(error),
        }
    }

    pub(crate) fn of_open_error(error: &io::Error, status_after_denial: impl FnOnce() -> io::Result<()>) -> Self {
        match error.raw_os_error() {
            Some(libc::EPERM) => match status_after_denial() {
                Ok(()) => Self::HiddenByPrivacy,
                Err(status_error) if status_error.raw_os_error() == Some(libc::EPERM) => Self::DataVault,
                Err(status_error) => Self::of_errno(&status_error),
            },
            Some(libc::EACCES) => Self::OtherUser,
            _ => Self::of_errno(error),
        }
    }

    fn of_errno(error: &io::Error) -> Self {
        match error.raw_os_error() {
            Some(libc::EDEADLK) => Self::InCloud,
            Some(libc::ENOENT) => Self::Vanished,
            Some(errno) => Self::Unreadable(errno),
            None => Self::Unreadable(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use domovoi_apfs_sys::AccessError;

    use super::NodeState;

    fn errno(code: i32) -> io::Error {
        io::Error::from_raw_os_error(code)
    }

    #[test]
    fn privacy_denial_depends_on_whether_stat_succeeds() {
        assert_eq!(
            NodeState::of_open_error(&errno(libc::EPERM), || Ok(())),
            NodeState::HiddenByPrivacy
        );
        assert_eq!(
            NodeState::of_open_error(&errno(libc::EPERM), || Err(errno(libc::EPERM))),
            NodeState::DataVault
        );
    }

    #[test]
    fn unix_denials_cloud_and_disappearance_are_classified() {
        assert_eq!(
            NodeState::of_open_error(&errno(libc::EACCES), || Ok(())),
            NodeState::OtherUser
        );
        assert_eq!(NodeState::of_listing_error(&errno(libc::EBADF)), NodeState::OtherUser);
        assert_eq!(NodeState::of_listing_error(&errno(libc::EDEADLK)), NodeState::InCloud);
        assert_eq!(NodeState::of_entry_error(&AccessError::Tcc), NodeState::HiddenByPrivacy);
        assert_eq!(NodeState::of_entry_error(&AccessError::Missing), NodeState::Vanished);
        assert_eq!(
            NodeState::of_listing_error(&errno(libc::EIO)),
            NodeState::Unreadable(libc::EIO)
        );
    }
}
