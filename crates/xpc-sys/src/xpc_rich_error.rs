use std::ffi::CStr;
use std::fmt;

use crate::sys::{xpc_release, xpc_rich_error_can_retry, xpc_rich_error_copy_description, xpc_rich_error_t};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XpcRichError {
    pub description: String,
    pub can_retry: bool,
}

impl XpcRichError {
    pub(crate) unsafe fn from_owned(raw: xpc_rich_error_t) -> Self {
        let error = unsafe { Self::from_borrowed(raw) };
        unsafe { xpc_release(raw.cast()) };
        error
    }

    pub(crate) unsafe fn from_borrowed(raw: xpc_rich_error_t) -> Self {
        assert!(!raw.is_null(), "XPC reported a failure without an error");
        let copied = unsafe { xpc_rich_error_copy_description(raw) };
        let description = unsafe { CStr::from_ptr(copied) }.to_string_lossy().into_owned();
        unsafe { libc::free(copied.cast()) };
        Self {
            description,
            can_retry: unsafe { xpc_rich_error_can_retry(raw) },
        }
    }

    pub(crate) fn rejected(description: &str) -> Self {
        Self {
            description: description.to_owned(),
            can_retry: false,
        }
    }
}

impl fmt::Display for XpcRichError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.description)
    }
}

impl std::error::Error for XpcRichError {}
