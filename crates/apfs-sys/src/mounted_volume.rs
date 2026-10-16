use std::ffi::{CStr, OsStr};
use std::io;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;

use crate::attribute_reader::invalid;
use crate::sys::MNT_EXT_ROOT_DATA_VOL;
use crate::syscall::check;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountedVolume {
    pub mount_point: PathBuf,
    pub device: PathBuf,
    pub file_system: String,
    pub root_data: bool,
}

pub fn mounted_volumes() -> io::Result<Vec<MountedVolume>> {
    let mounted = check(unsafe { libc::getfsstat(ptr::null_mut(), 0, libc::MNT_NOWAIT) })?;
    let mut mounts =
        vec![unsafe { mem::zeroed::<libc::statfs>() }; usize::try_from(mounted).expect("count is positive")];
    let capacity = libc::c_int::try_from(mounts.len() * size_of::<libc::statfs>())
        .map_err(|_| invalid("mount table is too large"))?;
    let filled = check(unsafe { libc::getfsstat(mounts.as_mut_ptr(), capacity, libc::MNT_NOWAIT) })?;
    mounts.truncate(usize::try_from(filled).expect("count is positive"));
    mounts
        .iter()
        .map(|mount| {
            Ok(MountedVolume {
                mount_point: PathBuf::from(OsStr::from_bytes(c_field(&mount.f_mntonname)?)),
                device: PathBuf::from(OsStr::from_bytes(c_field(&mount.f_mntfromname)?)),
                file_system: String::from_utf8_lossy(c_field(&mount.f_fstypename)?).into_owned(),
                root_data: mount.f_flags_ext & MNT_EXT_ROOT_DATA_VOL != 0,
            })
        })
        .collect()
}

fn c_field<const LENGTH: usize>(field: &[libc::c_char; LENGTH]) -> io::Result<&[u8]> {
    let bytes: &[u8; LENGTH] = unsafe { &*ptr::from_ref(field).cast() };
    CStr::from_bytes_until_nul(bytes)
        .map(CStr::to_bytes)
        .map_err(|_| invalid("statfs field is not NUL-terminated"))
}
