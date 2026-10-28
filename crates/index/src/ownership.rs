use crate::app::AppId;
use crate::index::Index;
use crate::node_id::NodeId;
use crate::owner_evidence::OwnerEvidence;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ownership {
    pub app: AppId,
    pub root: NodeId,
    pub evidence: OwnerEvidence,
}

impl Index {
    pub fn ownership(&self, node: NodeId) -> Option<Ownership> {
        let record = self.node(node);
        let evidence = record.owner_evidence?;
        let mut root = node;
        while let Some(parent) = self.parent(root) {
            let above = self.node(parent);
            if above.app != record.app || above.owner_evidence != record.owner_evidence {
                break;
            }
            root = parent;
        }
        Some(Ownership {
            app: record.app,
            root,
            evidence,
        })
    }

    pub fn owned_roots(&self, app: AppId) -> Vec<NodeId> {
        let mut roots = Vec::new();
        let mut pending = vec![self.root()];
        while let Some(current) = pending.pop() {
            if self.node(current).app == app {
                roots.push(current);
                continue;
            }
            pending.extend(self.children(current));
        }
        roots
    }
}
