use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::SystemTime;

use crate::attribute_reader::{AttributeReader, has, invalid, packed_group};
use crate::snapshot_class::SnapshotClass;
use crate::sys::{ATTR_BIT_MAP_COUNT, ATTR_CMN_CRTIME, ATTR_CMN_NAME, ATTR_CMN_RETURNED_ATTRS, fs_snapshot_list};
use crate::syscall::check;

const LISTING_CAPACITY: usize = 64 * 1024;
const LISTING_FLAGS: u32 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub created: SystemTime,
    pub class: SnapshotClass,
}

pub fn snapshots(volume_root: BorrowedFd<'_>) -> io::Result<Vec<Snapshot>> {
    let mut buffer = vec![0_u8; LISTING_CAPACITY];
    let mut snapshots = Vec::new();
    loop {
        let mut request = libc::attrlist {
            bitmapcount: u16::try_from(ATTR_BIT_MAP_COUNT).expect("the bitmap count fits in u16"),
            reserved: 0,
            commonattr: ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_NAME | ATTR_CMN_CRTIME,
            volattr: 0,
            dirattr: 0,
            fileattr: 0,
            forkattr: 0,
        };
        let count = check(unsafe {
            fs_snapshot_list(
                volume_root.as_raw_fd(),
                &raw mut request,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                LISTING_FLAGS,
            )
        })?;
        if count == 0 {
            return Ok(snapshots);
        }
        let mut position = 0;
        for _ in 0..count {
            let group = packed_group(&buffer, position)?;
            position += group.len();
            snapshots.push(parse(group)?);
        }
    }
}

fn parse(group: &[u8]) -> io::Result<Snapshot> {
    let mut reader = AttributeReader::new(group);
    let returned = reader.attribute_set()?;
    if !has(returned.commonattr, ATTR_CMN_NAME) || !has(returned.commonattr, ATTR_CMN_CRTIME) {
        return Err(invalid("snapshot entry lacks a name or a creation time"));
    }
    let name = reader
        .name()?
        .into_string()
        .map_err(|_| invalid("snapshot name is not UTF-8"))?;
    let created = reader.timestamp()?;
    Ok(Snapshot {
        class: SnapshotClass::of(&name),
        name,
        created,
    })
}
