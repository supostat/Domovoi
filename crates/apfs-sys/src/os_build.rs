use std::io;
use std::ptr;

use crate::attribute_reader::invalid;
use crate::syscall::check;

const OS_VERSION_KEY: &std::ffi::CStr = c"kern.osversion";

pub fn os_build_version() -> io::Result<String> {
    let mut length = 0;
    check(unsafe {
        libc::sysctlbyname(
            OS_VERSION_KEY.as_ptr(),
            ptr::null_mut(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    })?;
    let mut value = vec![0_u8; length];
    check(unsafe {
        libc::sysctlbyname(
            OS_VERSION_KEY.as_ptr(),
            value.as_mut_ptr().cast(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    })?;
    value.truncate(length);
    let text = value.split(|byte| *byte == 0).next().unwrap_or_default();
    String::from_utf8(text.to_vec()).map_err(|_| invalid("kern.osversion is not UTF-8"))
}
