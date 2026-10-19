use std::io;
use std::path::PathBuf;

use domovoi_index::{Index, NodeId};
use objc2_app_kit::NSWorkspace;

use crate::denylist::Denylist;
use crate::trash_failure::TrashFailure;
use crate::trash_journal::{TrashJournal, file_url};
use crate::trash_subject::TrashSubject;

const TRASH: &str = ".Trash";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItem {
    pub node: NodeId,
    pub path: PathBuf,
    pub physical: PathBuf,
    pub rule_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashOutcome {
    pub path: PathBuf,
    pub result: Result<PathBuf, TrashFailure>,
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

    pub fn move_to_trash(
        &mut self,
        index: &Index,
        denylist: &Denylist,
        journal: &mut TrashJournal,
    ) -> io::Result<Vec<TrashOutcome>> {
        let mut outcomes = Vec::with_capacity(self.items.len());
        for item in std::mem::take(&mut self.items) {
            if let Some(reason) = denylist.verdict(index, item.node) {
                outcomes.push(TrashOutcome {
                    path: item.path,
                    result: Err(TrashFailure::Denied(reason)),
                });
                continue;
            }
            let node = index.node(item.node);
            let result = journal.trash(&TrashSubject {
                original: item.path.clone(),
                original_physical: item.physical.clone(),
                location: item.physical,
                link_id: node.link_id,
                alloc_size: node.alloc_size,
                flags: node.flags,
                clone_id: node.clone_id,
                rule_id: item.rule_id,
            })?;
            outcomes.push(TrashOutcome {
                path: item.path,
                result,
            });
        }
        Ok(outcomes)
    }
}

pub fn open_trash() -> io::Result<bool> {
    let home =
        std::env::home_dir().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))?;
    let url =
        file_url(&home.join(TRASH)).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path is not UTF-8"))?;
    Ok(NSWorkspace::sharedWorkspace().openURL(&url))
}
