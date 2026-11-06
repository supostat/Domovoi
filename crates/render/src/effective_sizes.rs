use crate::diagram_tree::DiagramTree;
use crate::marked_nodes::MarkedNodes;

#[derive(Debug)]
pub struct EffectiveSizes<'a> {
    tree: &'a DiagramTree,
    deducted: Option<Vec<u64>>,
}

impl<'a> EffectiveSizes<'a> {
    pub fn of(tree: &'a DiagramTree, marked: &MarkedNodes, focus: usize) -> Self {
        if marked.is_empty() || marked.covers(tree, focus) {
            return Self { tree, deducted: None };
        }
        let mut sizes: Vec<u64> = (0..tree.len()).map(|node| tree.node(node).size).collect();
        for root in marked.roots() {
            deduct_from_ancestors(tree, &mut sizes, root, tree.node(root).size);
            empty_subtree(tree, &mut sizes, root);
        }
        for (node, bytes) in marked.deductions() {
            sizes[node] = sizes[node].saturating_sub(bytes);
            deduct_from_ancestors(tree, &mut sizes, node, bytes);
        }
        Self {
            tree,
            deducted: Some(sizes),
        }
    }

    pub fn size(&self, node: usize) -> u64 {
        self.deducted
            .as_ref()
            .map_or_else(|| self.tree.node(node).size, |sizes| sizes[node])
    }

    pub fn is_collapsed(&self, node: usize) -> bool {
        self.size(node) == 0 && self.tree.node(node).size > 0
    }
}

fn deduct_from_ancestors(tree: &DiagramTree, sizes: &mut [u64], node: usize, bytes: u64) {
    let mut current = tree.node(node).parent;
    while let Some(ancestor) = current {
        sizes[ancestor] = sizes[ancestor].saturating_sub(bytes);
        current = tree.node(ancestor).parent;
    }
}

fn empty_subtree(tree: &DiagramTree, sizes: &mut [u64], root: usize) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        sizes[node] = 0;
        pending.extend(tree.node(node).children.iter().copied());
    }
}
