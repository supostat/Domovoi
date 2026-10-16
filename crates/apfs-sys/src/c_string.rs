use std::ffi::{CString, OsStr};
use std::io;
use std::os::unix::ffi::OsStrExt;

pub(crate) fn c_string(text: &OsStr) -> io::Result<CString> {
    CString::new(text.as_bytes()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "name contains a NUL byte"))
}
