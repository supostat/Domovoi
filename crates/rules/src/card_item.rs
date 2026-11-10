use domovoi_index::{HeldGroup, NodeId, Reclaimable};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardItem {
    pub node: NodeId,
    pub reclaimable: Reclaimable,
    pub held: Vec<HeldGroup>,
}
