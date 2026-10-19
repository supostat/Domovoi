use std::ffi::OsStr;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use crate::c_string::c_string;
use crate::sys::{CLONE_NOFOLLOW, RENAME_EXCL, RENAME_SWAP, renameatx_np};
use crate::syscall::check;

pub fn open_at(directory: BorrowedFd<'_>, name: &OsStr, flags: libc::c_int, mode: libc::mode_t) -> io::Result<OwnedFd> {
    let name = c_string(name)?;
    let descriptor = check(unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC,
            libc::c_uint::from(mode),
        )
    })?;
    Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
}

pub fn open_search(directory: BorrowedFd<'_>, name: &OsStr) -> io::Result<OwnedFd> {
    open_at(directory, name, libc::O_SEARCH | libc::O_NOFOLLOW, 0)
}

pub fn stat_at(directory: BorrowedFd<'_>, name: &OsStr) -> io::Result<libc::stat> {
    let name = c_string(name)?;
    let mut status = MaybeUninit::<libc::stat>::uninit();
    check(unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            status.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    })?;
    Ok(unsafe { status.assume_init() })
}

pub fn mkdir_at(directory: BorrowedFd<'_>, name: &OsStr, mode: libc::mode_t) -> io::Result<()> {
    let name = c_string(name)?;
    check(unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), mode) }).map(drop)
}

pub fn clone_at(
    source_directory: BorrowedFd<'_>,
    source_name: &OsStr,
    target_directory: BorrowedFd<'_>,
    target_name: &OsStr,
) -> io::Result<()> {
    let source_name = c_string(source_name)?;
    let target_name = c_string(target_name)?;
    check(unsafe {
        libc::clonefileat(
            source_directory.as_raw_fd(),
            source_name.as_ptr(),
            target_directory.as_raw_fd(),
            target_name.as_ptr(),
            CLONE_NOFOLLOW,
        )
    })
    .map(drop)
}

pub fn rename_swap(
    first_directory: BorrowedFd<'_>,
    first_name: &OsStr,
    second_directory: BorrowedFd<'_>,
    second_name: &OsStr,
) -> io::Result<()> {
    let first_name = c_string(first_name)?;
    let second_name = c_string(second_name)?;
    check(unsafe {
        renameatx_np(
            first_directory.as_raw_fd(),
            first_name.as_ptr(),
            second_directory.as_raw_fd(),
            second_name.as_ptr(),
            RENAME_SWAP,
        )
    })
    .map(drop)
}

pub fn rename_exclusive(
    from_directory: BorrowedFd<'_>,
    from_name: &OsStr,
    to_directory: BorrowedFd<'_>,
    to_name: &OsStr,
) -> io::Result<()> {
    let from_name = c_string(from_name)?;
    let to_name = c_string(to_name)?;
    check(unsafe {
        renameatx_np(
            from_directory.as_raw_fd(),
            from_name.as_ptr(),
            to_directory.as_raw_fd(),
            to_name.as_ptr(),
            RENAME_EXCL,
        )
    })
    .map(drop)
}
