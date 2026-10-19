use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use domovoi_scanner::{Node, NodeState, ScanError, ScanOptions, list_directory, scan, scan_entry};

use crate::directory_listing::DirectoryListing;
use crate::listed_entry::ListedEntry;
use crate::node_id::NodeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeltaTarget {
    pub(crate) directory: NodeId,
    pub(crate) path: PathBuf,
    pub(crate) physical: PathBuf,
    pub(crate) subtree: bool,
    pub(crate) known_directories: Vec<(OsString, u64)>,
}

impl DeltaTarget {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn subtree(&self) -> bool {
        self.subtree
    }

    pub fn read(&self, options: &ScanOptions) -> Result<DirectoryListing, ScanError> {
        let (entries, descended) = if self.subtree {
            let children = scan(&self.physical, options)?.root.children;
            let descended = children.iter().map(listed_directories).sum();
            (children.into_iter().map(ListedEntry::Read).collect(), descended)
        } else {
            self.read_one_level(options)?
        };
        Ok(self.listing(entries, descended))
    }

    pub fn listing(&self, entries: Vec<ListedEntry>, descended: u64) -> DirectoryListing {
        DirectoryListing {
            directory: self.directory,
            path: self.path.clone(),
            subtree: self.subtree,
            entries,
            descended,
        }
    }

    fn read_one_level(&self, options: &ScanOptions) -> Result<(Vec<ListedEntry>, u64), ScanError> {
        let mut descended = 0;
        let mut entries = Vec::new();
        for entry in list_directory(&self.physical, options)? {
            if entry.state != NodeState::Listed {
                entries.push(ListedEntry::Read(entry));
            } else if self.keeps(&entry) {
                entries.push(ListedEntry::Kept(entry));
            } else {
                let scanned = scan_entry(&self.physical, entry, options)?;
                descended += listed_directories(&scanned);
                entries.push(ListedEntry::Read(scanned));
            }
        }
        Ok((entries, descended))
    }

    fn keeps(&self, entry: &Node) -> bool {
        let Some(file_id) = entry.attributes.as_ref().map(|attributes| attributes.file_id) else {
            return false;
        };
        self.known_directories
            .binary_search_by(|(name, _)| name.as_bytes().cmp(entry.name.as_bytes()))
            .is_ok_and(|position| self.known_directories[position].1 == file_id)
    }
}

fn listed_directories(node: &Node) -> u64 {
    let mut pending = vec![node];
    let mut count = 0;
    while let Some(current) = pending.pop() {
        if current.state == NodeState::Listed {
            count += 1;
        }
        pending.extend(&current.children);
    }
    count
}
