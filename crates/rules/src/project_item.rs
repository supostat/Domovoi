use domovoi_index::{NodeId, Reclaimable};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectItem {
    pub node: NodeId,
    pub rule: usize,
    pub reclaimable: Reclaimable,
}
