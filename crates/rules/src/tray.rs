use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use domovoi_index::{Index, NodeId, Reclaimable};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSFileManager;

use crate::denylist::Denylist;
use crate::trash_failure::TrashFailure;
use crate::trash_journal::{JournalEntry, TrashJournal, file_url};

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

    pub fn reclaimable(&self, index: &Index) -> io::Result<Reclaimable> {
        let mut total = Reclaimable::default();
        for item in &self.items {
            let item_reclaimable = index.reclaimable(item.node)?;
            total.bytes += item_reclaimable.bytes;
            total.held_by_clones += item_reclaimable.held_by_clones;
            total.held_by_snapshots += item_reclaimable.held_by_snapshots;
            total.held_by_hard_links += item_reclaimable.held_by_hard_links;
        }
        Ok(total)
    }

    pub fn move_to_trash(
        &mut self,
        index: &Index,
        denylist: &Denylist,
        journal: &mut TrashJournal,
    ) -> io::Result<Vec<TrashOutcome>> {
        let manager = NSFileManager::defaultManager();
        let mut outcomes = Vec::with_capacity(self.items.len());
        for item in std::mem::take(&mut self.items) {
            if let Some(reason) = denylist.verdict(index, item.node) {
                outcomes.push(TrashOutcome {
                    path: item.path,
                    result: Err(TrashFailure::Denied(reason)),
                });
                continue;
            }
            let url = file_url(&item.physical)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path is not UTF-8"))?;
            let mut resulting = None;
            let result = match manager.trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting)) {
                Ok(()) => {
                    let trashed = resulting
                        .and_then(|url| url.path())
                        .map(|path| PathBuf::from(path.to_string()))
                        .ok_or_else(|| io::Error::other("Finder did not report where the item went"))?;
                    let node = index.node(item.node);
                    journal.record(JournalEntry {
                        original: item.path.to_string_lossy().into_owned(),
                        original_physical: item.physical.to_string_lossy().into_owned(),
                        trashed: trashed.to_string_lossy().into_owned(),
                        link_id: node.link_id,
                        alloc_size: node.alloc_size,
                        flags: node.flags,
                        clone_id: node.clone_id,
                        rule_id: item.rule_id.clone(),
                        trashed_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_or(0, |since| since.as_secs()),
                    })?;
                    Ok(trashed)
                }
                Err(error) => Err(TrashFailure::from_cocoa(
                    error.code(),
                    error.localizedDescription().to_string(),
                )),
            };
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
