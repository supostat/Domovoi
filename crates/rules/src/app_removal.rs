use std::path::Path;

use domovoi_index::{Index, NodeId, OwnerEvidence};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalReason {
    Bundle,
    Owner(OwnerEvidence),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRemoval {
    pub bundle_id: String,
    pub name: String,
    pub items: Vec<(NodeId, RemovalReason)>,
    pub name_only: Vec<NodeId>,
}

impl AppRemoval {
    pub fn of(index: &Index, bundle_node: NodeId) -> Option<Self> {
        let canonical = index.path(bundle_node);
        let bundle = index
            .bundles()
            .iter()
            .find(|bundle| Path::new(&bundle.path) == canonical)?;
        let mut items = vec![(bundle_node, RemovalReason::Bundle)];
        let mut name_only = Vec::new();
        for root in index.owned_by_bundle(&bundle.path) {
            let evidence = index.ownership(root).expect("an owned root has an ownership").evidence;
            if evidence == OwnerEvidence::NameOnly {
                name_only.push(root);
            } else {
                items.push((root, RemovalReason::Owner(evidence)));
            }
        }
        Some(Self {
            bundle_id: bundle.bundle_id.clone(),
            name: bundle.name.clone(),
            items,
            name_only,
        })
    }
}
