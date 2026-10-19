use std::ffi::OsString;

use crate::kept_child::KeptChild;
use crate::node_id::NodeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplicedDirectory {
    pub directory: NodeId,
    pub kept: Vec<KeptChild>,
    pub appeared: Vec<OsString>,
    pub vanished: Vec<OsString>,
}
