use std::path::PathBuf;

use crate::listed_entry::ListedEntry;
use crate::node_id::NodeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryListing {
    pub(crate) directory: NodeId,
    pub path: PathBuf,
    pub subtree: bool,
    pub entries: Vec<ListedEntry>,
    pub descended: u64,
}
