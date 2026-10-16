use std::io;
use std::path::Path;

use crate::attribute_reader::AttributeReader;
use crate::c_string::c_string;
use crate::sys::{ATTR_BIT_MAP_COUNT, ATTR_CMN_RETURNED_ATTRS};
use crate::syscall::check;

const QUERY_CAPACITY: usize = 1024;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AttributeGroups {
    pub(crate) common: u32,
    pub(crate) volume: u32,
    pub(crate) extended: u32,
}

pub(crate) fn query_path<T>(
    path: &Path,
    groups: AttributeGroups,
    options: u32,
    parse: impl FnOnce(&mut AttributeReader<'_>, &libc::attribute_set_t) -> io::Result<T>,
) -> io::Result<T> {
    let path = c_string(path.as_os_str())?;
    let mut request = libc::attrlist {
        bitmapcount: u16::try_from(ATTR_BIT_MAP_COUNT).expect("the bitmap count fits in u16"),
        reserved: 0,
        commonattr: ATTR_CMN_RETURNED_ATTRS | groups.common,
        volattr: groups.volume,
        dirattr: 0,
        fileattr: 0,
        forkattr: groups.extended,
    };
    let mut buffer = vec![0_u8; QUERY_CAPACITY];
    check(unsafe {
        libc::getattrlist(
            path.as_ptr(),
            (&raw mut request).cast(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            options,
        )
    })?;
    let mut reader = AttributeReader::new(&buffer);
    let returned = reader.attribute_set()?;
    parse(&mut reader, &returned)
}

pub(crate) fn not_returned(description: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        format!("the file system did not return the {description}"),
    )
}
