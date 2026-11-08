use std::path::PathBuf;

use domovoi_index::NodeId;

use crate::member_stamp::MemberStamp;

#[derive(Debug)]
pub(crate) struct GroupPaths {
    pub(crate) size: u64,
    pub(crate) members: Vec<(NodeId, PathBuf, MemberStamp)>,
}
