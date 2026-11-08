use domovoi_index::NodeId;

use crate::deny_reason::DenyReason;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockedItem {
    pub node: NodeId,
    pub rule: usize,
    pub reason: DenyReason,
}
