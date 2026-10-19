use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::ops::AddAssign;

use domovoi_apfs_sys::private_size;
use domovoi_apfs_sys::sys::{EF_SHARES_ALL_BLOCKS, UF_COMPRESSED};

use crate::index::Index;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reclaimable {
    pub bytes: u64,
    pub held_by_clones: u64,
    pub held_by_snapshots: u64,
    pub held_by_hard_links: u64,
}

impl AddAssign for Reclaimable {
    fn add_assign(&mut self, other: Self) {
        self.bytes += other.bytes;
        self.held_by_clones += other.held_by_clones;
        self.held_by_snapshots += other.held_by_snapshots;
        self.held_by_hard_links += other.held_by_hard_links;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileFacts {
    pub(crate) compressed: bool,
    pub(crate) shares_all_blocks: bool,
    pub(crate) clone_refcount: u32,
    pub(crate) stream_inodes: u32,
    pub(crate) alloc_size: u64,
}

pub(crate) fn estimate_file(facts: FileFacts) -> Reclaimable {
    let regular_clone = !facts.compressed && facts.clone_refcount > 1;
    if facts.shares_all_blocks || facts.stream_inodes > 1 || regular_clone {
        Reclaimable {
            held_by_clones: facts.alloc_size,
            ..Reclaimable::default()
        }
    } else {
        Reclaimable {
            bytes: facts.alloc_size,
            ..Reclaimable::default()
        }
    }
}

pub(crate) fn reclaimable_file(
    facts: FileFacts,
    private_size: impl FnOnce() -> io::Result<u64>,
) -> io::Result<Reclaimable> {
    if facts.compressed {
        return Ok(estimate_file(facts));
    }
    let private = private_size()?.min(facts.alloc_size);
    let held = facts.alloc_size - private;
    let held_by_clone = facts.clone_refcount > 1 || facts.shares_all_blocks;
    Ok(Reclaimable {
        bytes: private,
        held_by_clones: if held_by_clone { held } else { 0 },
        held_by_snapshots: if held_by_clone { 0 } else { held },
        held_by_hard_links: 0,
    })
}

impl Index {
    pub fn reclaimable(&self, id: NodeId) -> io::Result<Reclaimable> {
        self.sum_over_inodes(id, |entry, facts| {
            let path = self.physical_path(entry);
            match reclaimable_file(facts, || private_size(&path)) {
                Err(error) if error.raw_os_error() == Some(libc::ENOENT) => Ok(Reclaimable::default()),
                outcome => outcome,
            }
        })
    }

    pub fn reclaimable_estimate(&self, id: NodeId) -> Reclaimable {
        let Ok(total) = self.sum_over_inodes(id, |_, facts| Ok::<_, Infallible>(estimate_file(facts)));
        total
    }

    fn sum_over_inodes<E>(
        &self,
        id: NodeId,
        mut reclaim: impl FnMut(NodeId, FileFacts) -> Result<Reclaimable, E>,
    ) -> Result<Reclaimable, E> {
        let mut total = Reclaimable::default();
        let mut linked_entries: HashMap<(u32, u64), Vec<NodeId>> = HashMap::new();
        for member in self.subtree(id) {
            let node = self.node(member);
            if node.kind != NodeKind::File {
                continue;
            }
            if node.link_count > 1 {
                linked_entries
                    .entry((node.real_dev, node.file_id))
                    .or_default()
                    .push(member);
            } else {
                total += reclaim(member, self.file_facts(member))?;
            }
        }
        for entries in linked_entries.values() {
            let node = self.node(entries[0]);
            if (node.link_count as usize) > entries.len() {
                total.held_by_hard_links += node.alloc_size;
            } else {
                total += reclaim(entries[0], self.file_facts(entries[0]))?;
            }
        }
        Ok(total)
    }

    fn file_facts(&self, id: NodeId) -> FileFacts {
        let node = self.node(id);
        FileFacts {
            compressed: node.flags & UF_COMPRESSED != 0,
            shares_all_blocks: node.ext_flags & u64::from(EF_SHARES_ALL_BLOCKS) != 0,
            clone_refcount: node.clone_refcount,
            stream_inodes: node.shares_with + 1,
            alloc_size: node.alloc_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{FileFacts, Reclaimable, reclaimable_file};

    const ALLOCATED: u64 = 40_960;

    fn facts() -> FileFacts {
        FileFacts {
            compressed: false,
            shares_all_blocks: false,
            clone_refcount: 1,
            stream_inodes: 1,
            alloc_size: ALLOCATED,
        }
    }

    #[test]
    fn blocks_kept_only_by_a_snapshot_are_held_by_the_snapshot() {
        let reclaimable = reclaimable_file(facts(), || Ok(0)).expect("facts are complete");
        assert_eq!(
            reclaimable,
            Reclaimable {
                held_by_snapshots: ALLOCATED,
                ..Reclaimable::default()
            }
        );
    }

    #[test]
    fn compressed_files_never_read_private_size() {
        let compressed = FileFacts {
            compressed: true,
            ..facts()
        };
        let unread = || -> io::Result<u64> { panic!("PRIVATESIZE is meaningless for decmpfs files") };
        assert_eq!(
            reclaimable_file(compressed, unread).expect("facts are complete").bytes,
            ALLOCATED
        );
        let cloned = FileFacts {
            stream_inodes: 2,
            ..compressed
        };
        assert_eq!(
            reclaimable_file(cloned, unread)
                .expect("facts are complete")
                .held_by_clones,
            ALLOCATED
        );
    }
}
