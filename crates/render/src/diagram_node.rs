use domovoi_index::Category;

use crate::node_origin::NodeOrigin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramNode {
    pub name: String,
    pub category: Category,
    pub size: u64,
    pub origin: NodeOrigin,
    pub depth: u32,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}
