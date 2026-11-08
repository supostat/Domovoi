use std::path::{Path, PathBuf};

use domovoi_index::{Index, NodeId, Reclaimable};

use crate::app_removal::{AppRemoval, RemovalReason};
use crate::delete_refusal::DeleteRefusal;
use crate::delete_subject::DeleteSubject;
use crate::denylist::Denylist;
use crate::tray_application::TrayApplication;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItem {
    pub node: NodeId,
    pub path: PathBuf,
    pub physical: PathBuf,
    pub rule_id: Option<String>,
    pub reason: Option<RemovalReason>,
    pub application: Option<String>,
    pub project: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tray {
    items: Vec<TrayItem>,
    applications: Vec<TrayApplication>,
}

impl Tray {
    pub fn add(&mut self, index: &Index, node: NodeId, rule_id: Option<&str>) {
        self.insert(index, node, rule_id.map(str::to_owned), None, None, None);
    }

    pub fn add_project_artifact(&mut self, index: &Index, node: NodeId, rule_id: &str, project: &Path) {
        self.insert(
            index,
            node,
            Some(rule_id.to_owned()),
            None,
            None,
            Some(project.to_path_buf()),
        );
    }

    pub fn add_application(&mut self, index: &Index, removal: &AppRemoval) {
        for (node, reason) in &removal.items {
            self.insert(index, *node, None, Some(*reason), Some(removal.bundle_id.clone()), None);
        }
        self.applications
            .retain(|application| application.bundle_id != removal.bundle_id);
        self.applications.push(TrayApplication {
            bundle_id: removal.bundle_id.clone(),
            name: removal.name.clone(),
            name_only: removal.name_only.iter().map(|node| index.path(*node)).collect(),
        });
    }

    fn insert(
        &mut self,
        index: &Index,
        node: NodeId,
        rule_id: Option<String>,
        reason: Option<RemovalReason>,
        application: Option<String>,
        project: Option<PathBuf>,
    ) {
        let path = index.path(node);
        if self.covers(&path) {
            return;
        }
        self.items.retain(|item| !item.path.starts_with(&path));
        self.items.push(TrayItem {
            node,
            physical: index.physical_path(node),
            path,
            rule_id,
            reason,
            application,
            project,
        });
    }

    pub fn remove(&mut self, node: NodeId) {
        self.items.retain(|item| item.node != node);
        let items = &self.items;
        self.applications.retain(|application| {
            items
                .iter()
                .any(|item| item.application.as_deref() == Some(application.bundle_id.as_str()))
        });
    }

    pub fn items(&self) -> &[TrayItem] {
        &self.items
    }

    pub fn covers(&self, path: &Path) -> bool {
        self.items.iter().any(|item| path.starts_with(&item.path))
    }

    pub fn applications(&self) -> &[TrayApplication] {
        &self.applications
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
        self.applications.clear();
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
                indexed_files: u64::from(index.node(item.node).files),
                rule_id: item.rule_id,
            });
        }
        (subjects, refusals)
    }
}
