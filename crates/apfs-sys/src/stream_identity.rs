use std::io;
use std::path::Path;

use crate::attribute_reader::has;
use crate::path_query::{AttributeGroups, not_returned, query_path};
use crate::sys::{
    ATTR_CMNEXT_CLONEID, ATTR_CMNEXT_EXT_FLAGS, ATTR_CMNEXT_REALDEVID, FSOPT_ATTR_CMN_EXTENDED, FSOPT_NOFOLLOW,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamIdentity {
    pub real_dev: u32,
    pub clone_id: u64,
    pub ext_flags: u64,
}

pub fn stream_identity(path: &Path) -> io::Result<StreamIdentity> {
    let groups = AttributeGroups {
        extended: ATTR_CMNEXT_REALDEVID | ATTR_CMNEXT_CLONEID | ATTR_CMNEXT_EXT_FLAGS,
        ..AttributeGroups::default()
    };
    query_path(
        path,
        groups,
        FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED,
        |reader, returned| {
            let expected = [
                (ATTR_CMNEXT_REALDEVID, "real device id"),
                (ATTR_CMNEXT_CLONEID, "clone id"),
                (ATTR_CMNEXT_EXT_FLAGS, "extended flags"),
            ];
            if let Some((_, description)) = expected
                .iter()
                .find(|(attribute, _)| !has(returned.forkattr, *attribute))
            {
                return Err(not_returned(description));
            }
            Ok(StreamIdentity {
                real_dev: reader.u32()?,
                clone_id: reader.u64()?,
                ext_flags: reader.u64()?,
            })
        },
    )
}
