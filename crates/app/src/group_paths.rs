use std::path::PathBuf;

use domovoi_index::NodeId;

#[derive(Debug)]
pub(crate) struct GroupPaths {
    pub(crate) size: u64,
    pub(crate) fully_hashed: bool,
    pub(crate) members: Vec<(NodeId, PathBuf)>,
}
