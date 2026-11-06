use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_tree::DiagramTree;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkedNodes {
    roots: BTreeSet<usize>,
    deductions: BTreeMap<usize, u64>,
}

impl MarkedNodes {
    pub fn mark(&mut self, node: usize) {
        self.roots.insert(node);
    }

    pub fn deduct(&mut self, node: usize, bytes: u64) {
        *self.deductions.entry(node).or_default() += bytes;
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty() && self.deductions.is_empty()
    }

    pub fn covers(&self, tree: &DiagramTree, node: usize) -> bool {
        let mut current = Some(node);
        while let Some(candidate) = current {
            if self.roots.contains(&candidate) {
                return true;
            }
            current = tree.node(candidate).parent;
        }
        false
    }

    pub fn roots(&self) -> impl Iterator<Item = usize> + '_ {
        self.roots.iter().copied()
    }

    pub fn deductions(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        self.deductions.iter().map(|(node, bytes)| (*node, *bytes))
    }
}
