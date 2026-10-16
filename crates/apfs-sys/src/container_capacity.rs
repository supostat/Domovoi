use std::io;
use std::path::Path;

use crate::attribute_reader::has;
use crate::path_query::{AttributeGroups, not_returned, query_path};
use crate::sys::{ATTR_VOL_INFO, ATTR_VOL_SIZE, ATTR_VOL_SPACEAVAIL};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerCapacity {
    pub size: u64,
    pub available: u64,
}

pub fn container_capacity(volume_root: &Path) -> io::Result<ContainerCapacity> {
    let groups = AttributeGroups {
        volume: ATTR_VOL_INFO | ATTR_VOL_SIZE | ATTR_VOL_SPACEAVAIL,
        ..AttributeGroups::default()
    };
    query_path(volume_root, groups, 0, |reader, returned| {
        if !has(returned.volattr, ATTR_VOL_SIZE) || !has(returned.volattr, ATTR_VOL_SPACEAVAIL) {
            return Err(not_returned("container size and available space"));
        }
        Ok(ContainerCapacity {
            size: reader.size()?,
            available: reader.size()?,
        })
    })
}
