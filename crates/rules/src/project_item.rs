use domovoi_index::{HeldGroup, NodeId, Reclaimable};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectItem {
    pub node: NodeId,
    pub rule: usize,
    pub reclaimable: Reclaimable,
    pub held: Vec<HeldGroup>,
}
