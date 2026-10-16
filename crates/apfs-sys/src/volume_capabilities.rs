use std::io;
use std::path::Path;

use crate::attribute_reader::has;
use crate::path_query::{AttributeGroups, not_returned, query_path};
use crate::sys::{
    ATTR_VOL_CAPABILITIES, ATTR_VOL_INFO, VOL_CAP_FMT_CASE_SENSITIVE, VOL_CAP_INT_CLONE, VOL_CAP_INT_RENAME_SWAP,
    VOL_CAPABILITIES_FORMAT, VOL_CAPABILITIES_INTERFACES,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeCapabilities {
    pub case_sensitive: bool,
    pub clone: bool,
    pub rename_swap: bool,
}

pub fn volume_capabilities(volume_root: &Path) -> io::Result<VolumeCapabilities> {
    let groups = AttributeGroups {
        volume: ATTR_VOL_INFO | ATTR_VOL_CAPABILITIES,
        ..AttributeGroups::default()
    };
    query_path(volume_root, groups, 0, |reader, returned| {
        if !has(returned.volattr, ATTR_VOL_CAPABILITIES) {
            return Err(not_returned("volume capabilities"));
        }
        let capabilities = reader.capabilities()?;
        let supports = |set: u32, capability: u32| {
            let index = set as usize;
            capabilities.capabilities[index] & capabilities.valid[index] & capability != 0
        };
        Ok(VolumeCapabilities {
            case_sensitive: supports(VOL_CAPABILITIES_FORMAT, VOL_CAP_FMT_CASE_SENSITIVE),
            clone: supports(VOL_CAPABILITIES_INTERFACES, VOL_CAP_INT_CLONE),
            rename_swap: supports(VOL_CAPABILITIES_INTERFACES, VOL_CAP_INT_RENAME_SWAP),
        })
    })
}
