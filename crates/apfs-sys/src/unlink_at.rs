use std::ffi::OsStr;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

use crate::c_string::c_string;
use crate::syscall::check;

pub fn unlink_at(directory: BorrowedFd<'_>, name: &OsStr, flags: libc::c_int) -> io::Result<()> {
    let name = c_string(name)?;
    check(unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), flags) }).map(drop)
}
