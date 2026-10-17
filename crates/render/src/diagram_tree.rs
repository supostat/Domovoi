use domovoi_index::{Index, NodeId};

use crate::diagram_node::DiagramNode;
use crate::diagram_outline::DiagramOutline;

pub const ROOT: usize = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramTree {
    nodes: Vec<DiagramNode>,
}

impl DiagramTree {
    pub fn from_outline(outline: &DiagramOutline) -> Self {
        let mut tree = Self { nodes: Vec::new() };
        tree.insert(outline, None, 0);
        tree
    }

    pub fn from_index(index: &Index, root: NodeId, depth: u32) -> Self {
        Self::from_outline(&DiagramOutline::indexed(index, root, &|_, level| level < depth))
    }

    pub fn node(&self, node: usize) -> &DiagramNode {
        &self.nodes[node]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn path(&self, node: usize) -> Vec<usize> {
        let mut path = vec![node];
        let mut current = node;
        while let Some(parent) = self.nodes[current].parent {
            path.push(parent);
            current = parent;
        }
        path.reverse();
        path
    }

    fn insert(&mut self, outline: &DiagramOutline, parent: Option<usize>, depth: u32) -> usize {
        let position = self.nodes.len();
        self.nodes.push(DiagramNode {
            name: outline.name.clone(),
            category: outline.category,
            size: outline.size,
            origin: outline.origin,
            depth,
            parent,
            children: Vec::new(),
        });
        let mut children: Vec<usize> = outline
            .children
            .iter()
            .map(|child| self.insert(child, Some(position), depth + 1))
            .collect();
        if !children.is_empty() {
            self.nodes[position].size = children.iter().map(|child| self.nodes[*child].size).sum();
            children.sort_by_key(|child| std::cmp::Reverse(self.nodes[*child].size));
        }
        self.nodes[position].children = children;
        position
    }
}
