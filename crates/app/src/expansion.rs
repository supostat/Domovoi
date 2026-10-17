use std::collections::{BinaryHeap, HashSet};

use domovoi_index::{Index, NodeId};
use domovoi_render::RING_COUNT;

pub const NODE_BUDGET: usize = 200_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expansion {
    expanded: HashSet<NodeId>,
}

impl Expansion {
    pub fn around(index: &Index, focus: NodeId, budget: usize) -> Self {
        let mut expansion = Self::default();
        let mut spent = 0;
        let mut chain = vec![focus];
        while let Some(parent) = index.parent(*chain.last().expect("the chain starts with the focus")) {
            chain.push(parent);
        }
        for node in chain.into_iter().rev() {
            spent += expansion.expand(index, node);
        }
        let mut level = vec![focus];
        for _ in 1..RING_COUNT {
            let mut next = Vec::new();
            for node in level {
                for child in index.children(node).filter(|child| is_expandable(index, *child)) {
                    if spent >= budget {
                        return expansion;
                    }
                    spent += expansion.expand(index, child);
                    next.push(child);
                }
            }
            level = next;
        }
        let mut largest: BinaryHeap<(u64, NodeId)> = index
            .children(index.root())
            .filter(|child| is_expandable(index, *child) && !expansion.expands(*child))
            .map(|child| (index.node(child).allocated, child))
            .collect();
        while let Some((_, node)) = largest.pop() {
            if spent >= budget {
                break;
            }
            spent += expansion.expand(index, node);
            largest.extend(
                index
                    .children(node)
                    .filter(|child| is_expandable(index, *child) && !expansion.expands(*child))
                    .map(|child| (index.node(child).allocated, child)),
            );
        }
        expansion
    }

    pub fn expands(&self, node: NodeId) -> bool {
        self.expanded.contains(&node)
    }

    pub fn shows_rings_below(&self, index: &Index, focus: NodeId) -> bool {
        let mut level = vec![focus];
        for _ in 0..RING_COUNT {
            let mut next = Vec::new();
            for node in level {
                if is_expandable(index, node) && !self.expands(node) {
                    return false;
                }
                next.extend(index.children(node));
            }
            level = next;
        }
        true
    }

    fn expand(&mut self, index: &Index, node: NodeId) -> usize {
        if self.expanded.insert(node) {
            index.node(node).child_count as usize
        } else {
            0
        }
    }
}

fn is_expandable(index: &Index, node: NodeId) -> bool {
    let record = index.node(node);
    record.child_count > 0 && record.allocated > 0
}
