use domovoi_index::{Category, Index, NodeId};

use crate::node_origin::NodeOrigin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramOutline {
    pub name: String,
    pub category: Category,
    pub size: u64,
    pub origin: NodeOrigin,
    pub children: Vec<DiagramOutline>,
}

impl DiagramOutline {
    pub fn indexed(index: &Index, node: NodeId, expands: &impl Fn(NodeId, u32) -> bool) -> Self {
        Self::indexed_at(index, node, 0, expands)
    }

    fn indexed_at(index: &Index, node: NodeId, depth: u32, expands: &impl Fn(NodeId, u32) -> bool) -> Self {
        let record = index.node(node);
        let children = if expands(node, depth) {
            index
                .children(node)
                .filter(|child| index.node(*child).allocated > 0)
                .map(|child| Self::indexed_at(index, child, depth + 1, expands))
                .collect()
        } else {
            Vec::new()
        };
        let name = if node == index.root() {
            index.path(node).to_string_lossy().into_owned()
        } else {
            index.name(node).to_string_lossy().into_owned()
        };
        Self {
            name,
            category: record.category,
            size: record.allocated,
            origin: NodeOrigin::Indexed(node),
            children,
        }
    }
}
