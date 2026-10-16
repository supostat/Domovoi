use crate::node::Node;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanReport {
    pub root: Node,
    pub files: u64,
    pub bytes: u64,
}
