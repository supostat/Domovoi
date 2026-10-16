use std::ffi::c_void;
use std::io;
use std::path::Path;

use objc2::rc::Retained;
use objc2_foundation::{NSString, NSURL};

const NO_ERROR: i32 = 0;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn CSBackupSetItemExcluded(item: *const c_void, exclude: u8, exclude_by_path: u8) -> i32;
    fn CSBackupIsItemExcluded(item: *const c_void, exclude_by_path: *mut u8) -> u8;
}

pub fn exclude_from_backup(path: &Path) -> io::Result<()> {
    let url = file_url(path)?;
    let status = unsafe { CSBackupSetItemExcluded(Retained::as_ptr(&url).cast(), 1, 0) };
    if status == NO_ERROR {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Time Machine refused to exclude {} (OSStatus {status})",
            path.display()
        )))
    }
}

pub fn is_excluded_from_backup(path: &Path) -> io::Result<bool> {
    let url = file_url(path)?;
    let mut by_path = 0_u8;
    Ok(unsafe { CSBackupIsItemExcluded(Retained::as_ptr(&url).cast(), &raw mut by_path) } != 0)
}

fn file_url(path: &Path) -> io::Result<Retained<NSURL>> {
    let text = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path is not UTF-8"))?;
    Ok(NSURL::fileURLWithPath(&NSString::from_str(text)))
}
