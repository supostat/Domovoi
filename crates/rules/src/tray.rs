use std::path::PathBuf;

use domovoi_index::{Index, NodeId, Reclaimable};

use crate::delete_refusal::DeleteRefusal;
use crate::delete_subject::DeleteSubject;
use crate::denylist::Denylist;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItem {
    pub node: NodeId,
    pub path: PathBuf,
    pub physical: PathBuf,
    pub rule_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tray {
    items: Vec<TrayItem>,
}

impl Tray {
    pub fn add(&mut self, index: &Index, node: NodeId, rule_id: Option<&str>) {
        let path = index.path(node);
        if self.items.iter().any(|item| path.starts_with(&item.path)) {
            return;
        }
        self.items.retain(|item| !item.path.starts_with(&path));
        self.items.push(TrayItem {
            node,
            physical: index.physical_path(node),
            path,
            rule_id: rule_id.map(str::to_owned),
        });
    }

    pub fn remove(&mut self, node: NodeId) {
        self.items.retain(|item| item.node != node);
    }

    pub fn items(&self) -> &[TrayItem] {
        &self.items
    }

    pub fn take_for_delete(
        &mut self,
        index: &Index,
        denylist: &Denylist,
        sizes: &[Reclaimable],
    ) -> (Vec<DeleteSubject>, Vec<DeleteRefusal>) {
        assert_eq!(sizes.len(), self.items.len(), "every item of the tray is measured");
        let mut subjects = Vec::with_capacity(self.items.len());
        let mut refusals = Vec::new();
        for (item, size) in std::mem::take(&mut self.items).into_iter().zip(sizes) {
            if let Some(reason) = denylist.verdict(index, item.node) {
                refusals.push(DeleteRefusal {
                    path: item.path,
                    reason,
                });
                continue;
            }
            subjects.push(DeleteSubject {
                path: item.path,
                physical: item.physical,
                link_id: index.node(item.node).link_id,
                measured_bytes: size.bytes,
                rule_id: item.rule_id,
            });
        }
        (subjects, refusals)
    }
}
