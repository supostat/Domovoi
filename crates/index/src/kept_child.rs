use crate::node_id::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptChild {
    pub previous: NodeId,
    pub current: NodeId,
}
