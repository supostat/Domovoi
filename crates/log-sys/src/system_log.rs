use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::NonNull;

const NUL_SYMBOL: &str = "\u{2400}";

unsafe extern "C" {
    fn domovoi_log_create(subsystem: *const c_char, category: *const c_char) -> *mut c_void;
    fn domovoi_log_release(log: *mut c_void);
    fn domovoi_log_notice(log: *mut c_void, message: *const c_char);
    fn domovoi_log_error(log: *mut c_void, message: *const c_char);
}

#[derive(Debug)]
pub struct SystemLog {
    handle: NonNull<c_void>,
}

unsafe impl Send for SystemLog {}
unsafe impl Sync for SystemLog {}

impl SystemLog {
    pub fn new(subsystem: &CStr, category: &CStr) -> Self {
        let handle = unsafe { domovoi_log_create(subsystem.as_ptr(), category.as_ptr()) };
        Self {
            handle: NonNull::new(handle).expect("os_log_create returns a log object"),
        }
    }

    pub fn notice(&self, message: &str) {
        let text = c_text(message);
        unsafe { domovoi_log_notice(self.handle.as_ptr(), text.as_ptr()) };
    }

    pub fn error(&self, message: &str) {
        let text = c_text(message);
        unsafe { domovoi_log_error(self.handle.as_ptr(), text.as_ptr()) };
    }
}

impl Drop for SystemLog {
    fn drop(&mut self) {
        unsafe { domovoi_log_release(self.handle.as_ptr()) };
    }
}

fn c_text(message: &str) -> CString {
    CString::new(message.replace('\0', NUL_SYMBOL)).expect("NUL bytes are replaced before the conversion")
}
