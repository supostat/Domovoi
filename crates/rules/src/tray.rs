use std::path::{Path, PathBuf};

use domovoi_index::{Index, NodeId, Reclaimable};

use crate::app_removal::{AppRemoval, RemovalReason};
use crate::delete_refusal::DeleteRefusal;
use crate::delete_subject::DeleteSubject;
use crate::deny_reason::DenyReason;
use crate::denylist::Denylist;
use crate::tray_application::TrayApplication;
use crate::tray_mark::TrayMark;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItem {
    pub node: NodeId,
    pub path: PathBuf,
    pub physical: PathBuf,
    pub rule_id: Option<String>,
    pub reason: Option<RemovalReason>,
    pub application: Option<String>,
    pub project: Option<PathBuf>,
    pub duplicate_of: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tray {
    items: Vec<TrayItem>,
    applications: Vec<TrayApplication>,
}

impl Tray {
    pub fn add(&mut self, index: &Index, node: NodeId, rule_id: Option<&str>) {
        self.insert(
            index,
            node,
            TrayMark {
                rule_id: rule_id.map(str::to_owned),
                ..TrayMark::default()
            },
        );
    }

    pub fn add_project_artifact(&mut self, index: &Index, node: NodeId, rule_id: &str, project: &Path) {
        self.insert(
            index,
            node,
            TrayMark {
                rule_id: Some(rule_id.to_owned()),
                project: Some(project.to_path_buf()),
                ..TrayMark::default()
            },
        );
    }

    pub fn add_duplicate_copy(&mut self, index: &Index, copy: NodeId, kept: NodeId) {
        self.insert(
            index,
            copy,
            TrayMark {
                duplicate_of: Some(index.path(kept)),
                ..TrayMark::default()
            },
        );
    }

    pub fn add_application(&mut self, index: &Index, removal: &AppRemoval) {
        for (node, reason) in &removal.items {
            self.insert(
                index,
                *node,
                TrayMark {
                    reason: Some(*reason),
                    application: Some(removal.bundle_id.clone()),
                    ..TrayMark::default()
                },
            );
        }
        self.applications
            .retain(|application| application.bundle_id != removal.bundle_id);
        self.applications.push(TrayApplication {
            bundle_id: removal.bundle_id.clone(),
            name: removal.name.clone(),
            name_only: removal.name_only.iter().map(|node| index.path(*node)).collect(),
        });
    }

    fn insert(&mut self, index: &Index, node: NodeId, mark: TrayMark) {
        let path = index.path(node);
        if self.covers(&path) {
            return;
        }
        self.items.retain(|item| !item.path.starts_with(&path));
        self.items.push(TrayItem {
            node,
            physical: index.physical_path(node),
            path,
            rule_id: mark.rule_id,
            reason: mark.reason,
            application: mark.application,
            project: mark.project,
            duplicate_of: mark.duplicate_of,
        });
    }

    #[must_use]
    pub fn remapped(&self, index: &Index, resolve: impl Fn(NodeId, &Path) -> Option<NodeId>) -> Self {
        let mut tray = Self {
            items: Vec::with_capacity(self.items.len()),
            applications: self.applications.clone(),
        };
        for item in &self.items {
            if let Some(node) = resolve(item.node, &item.path) {
                tray.insert(
                    index,
                    node,
                    TrayMark {
                        rule_id: item.rule_id.clone(),
                        reason: item.reason,
                        application: item.application.clone(),
                        project: item.project.clone(),
                        duplicate_of: item.duplicate_of.clone(),
                    },
                );
            }
        }
        tray.drop_applications_without_items();
        tray
    }

    pub fn remove(&mut self, node: NodeId) {
        self.items.retain(|item| item.node != node);
        self.drop_applications_without_items();
    }

    fn drop_applications_without_items(&mut self) {
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
        let verdicts: Vec<Result<Option<PathBuf>, DenyReason>> = self
            .items
            .iter()
            .map(|item| match denylist.verdict(index, item.node) {
                Some(reason) => Err(reason),
                None => self.kept_copy_of(index, item),
            })
            .collect();
        self.applications.clear();
        for ((item, size), verdict) in std::mem::take(&mut self.items).into_iter().zip(sizes).zip(verdicts) {
            let duplicate_of = match verdict {
                Ok(duplicate_of) => duplicate_of,
                Err(reason) => {
                    refusals.push(DeleteRefusal {
                        path: item.path,
                        reason,
                    });
                    continue;
                }
            };
            subjects.push(DeleteSubject {
                path: item.path,
                physical: item.physical,
                link_id: index.node(item.node).link_id,
                measured_bytes: size.bytes,
                indexed_files: u64::from(index.node(item.node).files),
                rule_id: item.rule_id,
                duplicate_of,
            });
        }
        (subjects, refusals)
    }

    fn kept_copy_of(&self, index: &Index, item: &TrayItem) -> Result<Option<PathBuf>, DenyReason> {
        let Some(kept) = &item.duplicate_of else {
            return Ok(None);
        };
        if self.covers(kept) {
            return Err(DenyReason::KeptCopyMarked);
        }
        index
            .find(kept)
            .map(|node| Some(index.physical_path(node)))
            .ok_or(DenyReason::KeptCopyMissing)
    }
}
