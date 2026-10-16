use std::fs::File;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;

use domovoi_apfs_sys::{Snapshot, container_capacity, mounted_volumes, snapshots, volume_used};

use crate::ffi::important_available_capacity;
use crate::index::Index;
use crate::measured::{Measured, Source};

const SYSTEM_ROOT: &str = "/";
const DEVICE_PREFIX: &str = "/dev/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volumes {
    pub data_used: Measured<u64>,
    pub system_used: Measured<u64>,
    pub other_volumes: Measured<u64>,
    pub container_size: Measured<u64>,
    pub free: Measured<u64>,
    pub purgeable: Measured<u64>,
    pub snapshots: Vec<Snapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HiddenSpace {
    pub total: Measured<i64>,
    pub purgeable: Measured<u64>,
    pub hidden_folders: u64,
    pub snapshots: Vec<Snapshot>,
    pub overhead: i64,
}

impl HiddenSpace {
    pub fn new(
        data_used: u64,
        visible: u64,
        purgeable: Measured<u64>,
        hidden_folders: u64,
        snapshots: Vec<Snapshot>,
    ) -> Self {
        let total = signed(data_used) - signed(visible);
        Self {
            total: Measured::new(total, Source::Getattrlist),
            purgeable,
            hidden_folders,
            snapshots,
            overhead: total - signed(purgeable.value),
        }
    }

    pub fn is_discrepancy(&self) -> bool {
        self.total.value < 0
    }
}

impl Volumes {
    pub fn measure(data_root: &Path) -> io::Result<Self> {
        let data_device = mounted_volumes()?
            .into_iter()
            .find(|volume| volume.mount_point == data_root)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "data volume is not mounted"))?
            .device;
        let container = container_of(&data_device)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "data volume device is not an APFS volume"))?
            .to_owned();
        let mut other_volumes = 0;
        for volume in mounted_volumes()? {
            let same_container = container_of(&volume.device) == Some(container.as_str());
            let counted_elsewhere = volume.mount_point == data_root || volume.mount_point == Path::new(SYSTEM_ROOT);
            if same_container && !counted_elsewhere {
                other_volumes += volume_used(&volume.mount_point)?;
            }
        }
        let capacity = container_capacity(data_root)?;
        let important = important_available_capacity(data_root)?;
        let volume_root = OwnedFd::from(File::open(data_root)?);
        Ok(Self {
            data_used: Measured::new(volume_used(data_root)?, Source::Getattrlist),
            system_used: Measured::new(volume_used(Path::new(SYSTEM_ROOT))?, Source::Getattrlist),
            other_volumes: Measured::new(other_volumes, Source::Getattrlist),
            container_size: Measured::new(capacity.size, Source::Getattrlist),
            free: Measured::new(capacity.available, Source::Getattrlist),
            purgeable: Measured::new(important.saturating_sub(capacity.available), Source::MacOsEstimate),
            snapshots: snapshots(volume_root.as_fd())?,
        })
    }

    pub fn hidden(&self, index: &Index) -> HiddenSpace {
        HiddenSpace::new(
            self.data_used.value,
            index.node(index.root()).allocated,
            self.purgeable,
            index.hidden_folders(),
            self.snapshots.clone(),
        )
    }
}

fn container_of(device: &Path) -> Option<&str> {
    let name = device.to_str()?.strip_prefix(DEVICE_PREFIX)?;
    let digits_end = name
        .strip_prefix("disk")?
        .find(|character: char| !character.is_ascii_digit())
        .map_or(name.len(), |position| position + "disk".len());
    (digits_end > "disk".len()).then(|| &name[..digits_end])
}

fn signed(value: u64) -> i64 {
    i64::try_from(value).expect("volume sizes fit in i64")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::container_of;

    #[test]
    fn volumes_of_one_container_share_its_disk() {
        assert_eq!(container_of(Path::new("/dev/disk3s5")), Some("disk3"));
        assert_eq!(container_of(Path::new("/dev/disk3s1s1")), Some("disk3"));
        assert_eq!(container_of(Path::new("/dev/disk12")), Some("disk12"));
        assert_eq!(container_of(Path::new("map auto_home")), None);
    }
}
