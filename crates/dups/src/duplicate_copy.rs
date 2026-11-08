use domovoi_index::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuplicateCopy {
    pub node: NodeId,
    pub shares_blocks: bool,
    pub package: Option<NodeId>,
}

impl DuplicateCopy {
    pub fn is_removable(&self) -> bool {
        !self.shares_blocks && self.package.is_none()
    }
}
