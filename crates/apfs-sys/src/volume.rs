use std::fmt::Write;
use std::io;
use std::path::{Path, PathBuf};

use crate::attribute_reader::has;
use crate::mounted_volume::mounted_volumes;
use crate::path_query::{AttributeGroups, not_returned, query_path};
use crate::sys::{ATTR_VOL_INFO, ATTR_VOL_SPACEUSED, ATTR_VOL_UUID};

const UUID_BYTES: usize = 16;
const UUID_GROUPS: [usize; 5] = [4, 2, 2, 2, 6];

pub fn volume_used(volume_root: &Path) -> io::Result<u64> {
    let groups = AttributeGroups {
        volume: ATTR_VOL_INFO | ATTR_VOL_SPACEUSED,
        ..AttributeGroups::default()
    };
    query_path(volume_root, groups, 0, |reader, returned| {
        if !has(returned.volattr, ATTR_VOL_SPACEUSED) {
            return Err(not_returned("used space of the volume"));
        }
        reader.size()
    })
}

pub fn volume_uuid(volume_root: &Path) -> io::Result<String> {
    let groups = AttributeGroups {
        volume: ATTR_VOL_INFO | ATTR_VOL_UUID,
        ..AttributeGroups::default()
    };
    query_path(volume_root, groups, 0, |reader, returned| {
        if !has(returned.volattr, ATTR_VOL_UUID) {
            return Err(not_returned("volume UUID"));
        }
        let mut bytes = Vec::with_capacity(UUID_BYTES);
        for _ in 0..UUID_BYTES / 4 {
            bytes.extend_from_slice(&reader.u32()?.to_ne_bytes());
        }
        Ok(format_uuid(&bytes))
    })
}

pub fn data_volume() -> io::Result<PathBuf> {
    mounted_volumes()?
        .into_iter()
        .find(|volume| volume.root_data)
        .map(|volume| volume.mount_point)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no mounted root data volume"))
}

fn format_uuid(bytes: &[u8]) -> String {
    let mut formatted = String::with_capacity(UUID_BYTES * 2 + UUID_GROUPS.len() - 1);
    let mut position = 0;
    for (index, group) in UUID_GROUPS.iter().enumerate() {
        if index > 0 {
            formatted.push('-');
        }
        for byte in &bytes[position..position + group] {
            write!(formatted, "{byte:02X}").expect("writing to a String succeeds");
        }
        position += group;
    }
    formatted
}
