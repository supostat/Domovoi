use std::io;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::mounted_volumes;

pub(crate) fn mount_point_of(physical: &Path) -> io::Result<PathBuf> {
    mounted_volumes()?
        .into_iter()
        .map(|volume| volume.mount_point)
        .filter(|mount_point| physical.starts_with(mount_point))
        .max_by_key(|mount_point| mount_point.components().count())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no mounted volume holds the path"))
}
