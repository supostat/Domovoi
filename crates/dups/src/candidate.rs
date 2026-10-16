use std::path::PathBuf;

use domovoi_index::NodeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) size: u64,
    pub(crate) entries: Vec<NodeId>,
    pub(crate) physical: PathBuf,
    pub(crate) in_cloud_folder: bool,
}
