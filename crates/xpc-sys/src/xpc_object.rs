use std::fmt;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::ptr::{self, NonNull};

use crate::sys::{_xpc_type_s, xpc_fd_create, xpc_get_type, xpc_object_t, xpc_release, xpc_retain};

pub struct XpcObject {
    raw: NonNull<std::ffi::c_void>,
}

unsafe impl Send for XpcObject {}
unsafe impl Sync for XpcObject {}

impl XpcObject {
    pub(crate) unsafe fn from_owned(raw: xpc_object_t) -> Self {
        Self {
            raw: NonNull::new(raw).expect("XPC returned an object"),
        }
    }

    pub(crate) unsafe fn from_borrowed(raw: xpc_object_t) -> Self {
        unsafe { Self::from_owned(xpc_retain(raw)) }
    }

    pub fn file_descriptor(file: BorrowedFd<'_>) -> io::Result<Self> {
        let raw = unsafe { xpc_fd_create(file.as_raw_fd()) };
        if raw.is_null() {
            return Err(io::Error::other("xpc_fd_create could not wrap the descriptor"));
        }
        Ok(unsafe { Self::from_owned(raw) })
    }

    pub fn as_raw(&self) -> xpc_object_t {
        self.raw.as_ptr()
    }

    pub(crate) fn has_type(&self, kind: &_xpc_type_s) -> bool {
        ptr::eq(unsafe { xpc_get_type(self.as_raw()) }, kind)
    }
}

impl Clone for XpcObject {
    fn clone(&self) -> Self {
        unsafe { Self::from_borrowed(self.as_raw()) }
    }
}

impl Drop for XpcObject {
    fn drop(&mut self) {
        unsafe { xpc_release(self.as_raw()) };
    }
}

impl fmt::Debug for XpcObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "XpcObject({:p})", self.raw)
    }
}
