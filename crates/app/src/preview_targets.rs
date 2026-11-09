use std::path::PathBuf;

use domovoi_index::Index;
use domovoi_render::{DiagramTree, NodeOrigin};

use crate::duplicate_key::DuplicateKey;
use crate::group_model::GroupModel;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewTargets {
    pub paths: Vec<PathBuf>,
}

impl PreviewTargets {
    pub fn from_level(tree: &DiagramTree, index: &Index, nodes: &[usize]) -> Self {
        Self {
            paths: nodes
                .iter()
                .filter_map(|node| match tree.node(*node).origin {
                    NodeOrigin::Indexed(id) => Some(index.path(id)),
                    NodeOrigin::Measured(_) | NodeOrigin::Outline => None,
                })
                .collect(),
        }
    }

    pub fn from_duplicates(groups: &[GroupModel], index: &Index, keys: &[DuplicateKey]) -> Self {
        Self {
            paths: keys
                .iter()
                .filter_map(|key| {
                    let group = groups.get(key.group)?;
                    let member = match key.member? {
                        0 => &group.keep,
                        copy => group.copies.get(copy - 1)?,
                    };
                    Some(index.path(member.node?))
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}
