use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::SystemTime;

use crate::attribute_reader::invalid;
use crate::syscall::check;
use crate::timestamp::{system_time, timespec_parts};

const LAST_USED_DATE: &std::ffi::CStr = c"com.apple.lastuseddate#PS";
const TIMESPEC_BYTES: usize = 16;

pub fn last_used_xattr(file: BorrowedFd<'_>) -> io::Result<Option<SystemTime>> {
    let mut value = [0_u8; TIMESPEC_BYTES];
    let read = unsafe {
        libc::fgetxattr(
            file.as_raw_fd(),
            LAST_USED_DATE.as_ptr(),
            value.as_mut_ptr().cast(),
            value.len(),
            0,
            0,
        )
    };
    if read == -1 {
        let error = io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(libc::ENOATTR) => Ok(None),
            _ => Err(error),
        };
    }
    if usize::try_from(read).ok() != Some(TIMESPEC_BYTES) {
        return Err(invalid("last used date is not a 16-byte timespec"));
    }
    let (seconds, nanoseconds) = value.split_at(TIMESPEC_BYTES / 2);
    let seconds = i64::from_le_bytes(seconds.try_into().expect("eight bytes"));
    let nanoseconds = i64::from_le_bytes(nanoseconds.try_into().expect("eight bytes"));
    system_time(seconds, nanoseconds).map(Some)
}

pub fn set_last_used_xattr(file: BorrowedFd<'_>, time: SystemTime) -> io::Result<()> {
    let (seconds, nanoseconds) = timespec_parts(time);
    let mut value = [0_u8; TIMESPEC_BYTES];
    value[..TIMESPEC_BYTES / 2].copy_from_slice(&seconds.to_le_bytes());
    value[TIMESPEC_BYTES / 2..].copy_from_slice(&nanoseconds.to_le_bytes());
    check(unsafe {
        libc::fsetxattr(
            file.as_raw_fd(),
            LAST_USED_DATE.as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
            0,
        )
    })
    .map(drop)
}
