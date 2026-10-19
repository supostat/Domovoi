use std::path::PathBuf;

use crate::listed_entry::ListedEntry;
use crate::node_id::NodeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryListing {
    pub(crate) directory: NodeId,
    pub path: PathBuf,
    pub subtree: bool,
    pub entries: Vec<ListedEntry>,
    pub descended: u64,
}

impl DirectoryListing {
    pub fn nodes(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| match entry {
                ListedEntry::Kept(_) => 1,
                ListedEntry::Read(node) => {
                    let mut count = 0;
                    let mut pending = vec![node];
                    while let Some(current) = pending.pop() {
                        count += 1;
                        pending.extend(&current.children);
                    }
                    count
                }
            })
            .sum()
    }
}
