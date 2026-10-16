use std::io;
use std::path::Path;

use crate::attribute_reader::has;
use crate::path_query::{AttributeGroups, not_returned, query_path};
use crate::sys::{
    ATTR_CMN_GEN_COUNT, ATTR_CMNEXT_PRIVATESIZE, ATTR_CMNEXT_REALDEVID, FSOPT_ATTR_CMN_EXTENDED, FSOPT_NOFOLLOW,
};

pub fn real_dev(path: &Path) -> io::Result<u32> {
    let groups = AttributeGroups {
        extended: ATTR_CMNEXT_REALDEVID,
        ..AttributeGroups::default()
    };
    query_path(
        path,
        groups,
        FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED,
        |reader, returned| {
            if !has(returned.forkattr, ATTR_CMNEXT_REALDEVID) {
                return Err(not_returned("real device id"));
            }
            reader.u32()
        },
    )
}

pub fn private_size(path: &Path) -> io::Result<u64> {
    let groups = AttributeGroups {
        extended: ATTR_CMNEXT_PRIVATESIZE,
        ..AttributeGroups::default()
    };
    query_path(
        path,
        groups,
        FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED,
        |reader, returned| {
            if !has(returned.forkattr, ATTR_CMNEXT_PRIVATESIZE) {
                return Err(not_returned("private size"));
            }
            reader.size()
        },
    )
}

pub fn gen_count(path: &Path) -> io::Result<u32> {
    let groups = AttributeGroups {
        common: ATTR_CMN_GEN_COUNT,
        ..AttributeGroups::default()
    };
    query_path(
        path,
        groups,
        FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED,
        |reader, returned| {
            if !has(returned.commonattr, ATTR_CMN_GEN_COUNT) {
                return Err(not_returned("generation count"));
            }
            reader.u32()
        },
    )
}
