use domovoi_index::{NodeId, Reclaimable};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardItem {
    pub node: NodeId,
    pub reclaimable: Reclaimable,
}
