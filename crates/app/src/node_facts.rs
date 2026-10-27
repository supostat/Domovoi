use std::time::SystemTime;

use domovoi_index::Index;
use domovoi_render::NodeOrigin;

use crate::hover_tip::HoverTip;
use crate::index_facts::IndexFacts;
use crate::shell_data::ShellData;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeFacts {
    pub node: usize,
    pub tip: HoverTip,
    pub indexed: Option<IndexFacts>,
}

impl NodeFacts {
    pub fn for_node(data: &ShellData, index: Option<&Index>, node: usize, now: SystemTime) -> Option<Self> {
        let tip = HoverTip::for_node(data, index, node)?;
        let indexed = match (data.tree.node(node).origin, index) {
            (NodeOrigin::Indexed(id), Some(index)) => Some(IndexFacts::of(index, id, now)),
            _ => None,
        };
        Some(Self { node, tip, indexed })
    }
}
