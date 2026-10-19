use std::io;
use std::path::Path;

use crate::attribute_set::AttributeSet;
use crate::bulk_entry::BulkEntry;
use crate::bulk_listing::request;
use crate::c_string::c_string;
use crate::sys::{ATTR_CMN_ERROR, FSOPT_ATTR_CMN_EXTENDED, FSOPT_NOFOLLOW};
use crate::syscall::check;

const ENTRY_CAPACITY: usize = 4096;

pub fn path_entry(path: &Path, attributes: AttributeSet) -> io::Result<BulkEntry> {
    let path = c_string(path.as_os_str())?;
    let mut request = request(attributes);
    request.commonattr &= !ATTR_CMN_ERROR;
    request.dirattr = 0;
    let mut buffer = vec![0_u8; ENTRY_CAPACITY];
    check(unsafe {
        libc::getattrlist(
            path.as_ptr(),
            (&raw mut request).cast(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED,
        )
    })?;
    BulkEntry::parse(&buffer)
}
