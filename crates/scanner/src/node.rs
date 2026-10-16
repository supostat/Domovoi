use std::ffi::OsString;
use std::time::SystemTime;

use domovoi_apfs_sys::EntryAttributes;

use crate::node_state::NodeState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub name: OsString,
    pub attributes: Option<EntryAttributes>,
    pub last_used: Option<SystemTime>,
    pub state: NodeState,
    pub children: Vec<Node>,
}
