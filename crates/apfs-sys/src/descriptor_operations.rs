use std::ffi::OsStr;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::ptr;

use crate::c_string::c_string;
use crate::sys::{COPYFILE_METADATA, COPYFILE_NOFOLLOW};
use crate::syscall::check;

pub fn pwrite_at(file: BorrowedFd<'_>, bytes: &[u8], offset: u64) -> io::Result<()> {
    let mut written = 0;
    while written < bytes.len() {
        let remaining = &bytes[written..];
        let position = offset
            .checked_add(u64::try_from(written).expect("usize fits in u64"))
            .and_then(|position| libc::off_t::try_from(position).ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "write offset overflows off_t"))?;
        let result = unsafe { libc::pwrite(file.as_raw_fd(), remaining.as_ptr().cast(), remaining.len(), position) };
        match result {
            -1 => {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::EINTR) {
                    return Err(error);
                }
            }
            0 => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            count => written += usize::try_from(count).expect("a positive count fits in usize"),
        }
    }
    Ok(())
}

pub fn ftruncate(file: BorrowedFd<'_>, length: u64) -> io::Result<()> {
    let length = libc::off_t::try_from(length)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "length overflows off_t"))?;
    check(unsafe { libc::ftruncate(file.as_raw_fd(), length) }).map(drop)
}

pub fn set_nocache(file: BorrowedFd<'_>) -> io::Result<()> {
    check(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) }).map(drop)
}

pub fn set_xattr(file: BorrowedFd<'_>, name: &OsStr, value: &[u8]) -> io::Result<()> {
    let name = c_string(name)?;
    check(unsafe {
        libc::fsetxattr(
            file.as_raw_fd(),
            name.as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
            0,
        )
    })
    .map(drop)
}

pub fn chflags(file: BorrowedFd<'_>, flags: u32) -> io::Result<()> {
    check(unsafe { libc::fchflags(file.as_raw_fd(), flags) }).map(drop)
}

pub fn copy_metadata(source: BorrowedFd<'_>, target: BorrowedFd<'_>) -> io::Result<()> {
    check(unsafe {
        libc::fcopyfile(
            source.as_raw_fd(),
            target.as_raw_fd(),
            ptr::null_mut(),
            COPYFILE_METADATA | COPYFILE_NOFOLLOW,
        )
    })
    .map(drop)
}
