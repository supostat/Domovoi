use domovoi_index::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuplicateCopy {
    pub node: NodeId,
    pub shares_blocks: bool,
}
