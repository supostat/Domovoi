use crate::index::Index;
use crate::node_id::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SafetyLevel {
    Safe,
    Recoverable,
    CheckYourself,
}

pub trait SafetyLevels {
    fn level(&self, index: &Index, node: NodeId) -> Option<SafetyLevel>;
}
