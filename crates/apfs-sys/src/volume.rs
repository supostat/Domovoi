use std::ffi::{CStr, OsStr};
use std::io;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

use crate::attribute_reader::{has, invalid};
use crate::path_query::{AttributeGroups, not_returned, query_path};
use crate::sys::{ATTR_VOL_INFO, ATTR_VOL_SPACEUSED, MNT_EXT_ROOT_DATA_VOL};
use crate::syscall::check;

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

pub fn data_volume() -> io::Result<PathBuf> {
    let mounted = check(unsafe { libc::getfsstat(ptr::null_mut(), 0, libc::MNT_NOWAIT) })?;
    let mut mounts =
        vec![unsafe { mem::zeroed::<libc::statfs>() }; usize::try_from(mounted).expect("count is positive")];
    let capacity = libc::c_int::try_from(mounts.len() * size_of::<libc::statfs>())
        .map_err(|_| invalid("mount table is too large"))?;
    let filled = check(unsafe { libc::getfsstat(mounts.as_mut_ptr(), capacity, libc::MNT_NOWAIT) })?;
    mounts.truncate(usize::try_from(filled).expect("count is positive"));
    let data = mounts
        .iter()
        .find(|mount| mount.f_flags_ext & MNT_EXT_ROOT_DATA_VOL != 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no mounted root data volume"))?;
    let mount_point = data.f_mntonname.map(|character| character as u8);
    let mount_point =
        CStr::from_bytes_until_nul(&mount_point).map_err(|_| invalid("mount point is not NUL-terminated"))?;
    Ok(PathBuf::from(OsStr::from_bytes(mount_point.to_bytes())))
}
