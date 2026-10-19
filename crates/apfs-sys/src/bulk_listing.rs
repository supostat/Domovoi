use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

use crate::attribute_reader::packed_group;
use crate::attribute_set::AttributeSet;
use crate::bulk_entry::BulkEntry;
use crate::sys::{
    ATTR_BIT_MAP_COUNT, ATTR_CMN_ADDEDTIME, ATTR_CMN_CRTIME, ATTR_CMN_ERROR, ATTR_CMN_FILEID, ATTR_CMN_FLAGS,
    ATTR_CMN_MODTIME, ATTR_CMN_NAME, ATTR_CMN_OBJTYPE, ATTR_CMN_RETURNED_ATTRS, ATTR_CMNEXT_CLONE_REFCNT,
    ATTR_CMNEXT_CLONEID, ATTR_CMNEXT_EXT_FLAGS, ATTR_CMNEXT_LINKID, ATTR_CMNEXT_PRIVATESIZE, ATTR_CMNEXT_REALDEVID,
    ATTR_DIR_MOUNTSTATUS, ATTR_FILE_ALLOCSIZE, ATTR_FILE_DATAALLOCSIZE, ATTR_FILE_LINKCOUNT, ATTR_FILE_TOTALSIZE,
    FSOPT_ATTR_CMN_EXTENDED,
};

const INITIAL_CAPACITY: usize = 256 * 1024;
const EXTENDED_ATTRIBUTES: u32 =
    ATTR_CMNEXT_REALDEVID | ATTR_CMNEXT_LINKID | ATTR_CMNEXT_CLONEID | ATTR_CMNEXT_EXT_FLAGS | ATTR_CMNEXT_CLONE_REFCNT;
const MAXIMUM_CAPACITY: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct BulkListing<'directory> {
    directory: BorrowedFd<'directory>,
    attributes: AttributeSet,
    buffer: Vec<u8>,
    position: usize,
    entries_left: u32,
    finished: bool,
}

impl<'directory> BulkListing<'directory> {
    pub fn new(directory: BorrowedFd<'directory>, attributes: AttributeSet) -> Self {
        Self::with_capacity(directory, attributes, INITIAL_CAPACITY)
    }

    fn with_capacity(directory: BorrowedFd<'directory>, attributes: AttributeSet, capacity: usize) -> Self {
        Self {
            directory,
            attributes,
            buffer: vec![0; capacity],
            position: 0,
            entries_left: 0,
            finished: false,
        }
    }

    fn fill(&mut self) -> io::Result<bool> {
        loop {
            let mut request = request(self.attributes);
            let count = unsafe {
                libc::getattrlistbulk(
                    self.directory.as_raw_fd(),
                    (&raw mut request).cast(),
                    self.buffer.as_mut_ptr().cast(),
                    self.buffer.len(),
                    u64::from(FSOPT_ATTR_CMN_EXTENDED),
                )
            };
            if count >= 0 {
                self.entries_left = u32::try_from(count).expect("a non-negative entry count fits in u32");
                self.position = 0;
                return Ok(count > 0);
            }
            let error = io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::EINTR) => {}
                Some(libc::ERANGE) if self.buffer.len() < MAXIMUM_CAPACITY => {
                    self.buffer.resize(self.buffer.len() * 2, 0);
                }
                _ => return Err(error),
            }
        }
    }

    fn next_group(&mut self) -> io::Result<&[u8]> {
        let group = packed_group(&self.buffer, self.position)?;
        self.position += group.len();
        self.entries_left -= 1;
        Ok(group)
    }
}

impl Iterator for BulkListing<'_> {
    type Item = io::Result<BulkEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        if self.entries_left == 0 {
            match self.fill() {
                Ok(true) => {}
                Ok(false) => {
                    self.finished = true;
                    return None;
                }
                Err(error) => {
                    self.finished = true;
                    return Some(Err(error));
                }
            }
        }
        let entry = self.next_group().and_then(BulkEntry::parse);
        if entry.is_err() {
            self.finished = true;
        }
        Some(entry)
    }
}

pub(crate) fn request(attributes: AttributeSet) -> libc::attrlist {
    let extended = match attributes {
        AttributeSet::Base => 0,
        AttributeSet::Extended => EXTENDED_ATTRIBUTES,
        AttributeSet::ExtendedWithPrivateSize => EXTENDED_ATTRIBUTES | ATTR_CMNEXT_PRIVATESIZE,
    };
    libc::attrlist {
        bitmapcount: u16::try_from(ATTR_BIT_MAP_COUNT).expect("the bitmap count fits in u16"),
        reserved: 0,
        commonattr: ATTR_CMN_RETURNED_ATTRS
            | ATTR_CMN_NAME
            | ATTR_CMN_ERROR
            | ATTR_CMN_OBJTYPE
            | ATTR_CMN_FLAGS
            | ATTR_CMN_FILEID
            | ATTR_CMN_MODTIME
            | ATTR_CMN_CRTIME
            | ATTR_CMN_ADDEDTIME,
        volattr: 0,
        dirattr: ATTR_DIR_MOUNTSTATUS,
        fileattr: ATTR_FILE_LINKCOUNT | ATTR_FILE_TOTALSIZE | ATTR_FILE_ALLOCSIZE | ATTR_FILE_DATAALLOCSIZE,
        forkattr: extended,
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::fd::AsFd;

    use super::BulkListing;
    use crate::attribute_set::AttributeSet;

    #[test]
    fn grows_a_buffer_too_small_for_one_entry() {
        let directory = std::env::temp_dir().join(format!("domovoi-bulk-growth-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory is created");
        fs::write(directory.join("a".repeat(200)), b"content").expect("file is written");
        let handle = File::open(&directory).expect("directory opens");
        let names: Vec<_> = BulkListing::with_capacity(handle.as_fd(), AttributeSet::Extended, 16)
            .map(|entry| entry.expect("entry parses").name)
            .collect();
        fs::remove_dir_all(&directory).expect("temp directory is removed");
        assert_eq!(names, vec![std::ffi::OsString::from("a".repeat(200))]);
    }
}
