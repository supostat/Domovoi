use std::collections::HashMap;
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

pub(crate) fn reclaimable_file(
    facts: FileFacts,
    private_size: impl FnOnce() -> io::Result<u64>,
) -> io::Result<Reclaimable> {
    if facts.compressed {
        return Ok(if facts.shares_all_blocks || facts.stream_inodes > 1 {
            Reclaimable {
                held_by_clones: facts.alloc_size,
                ..Reclaimable::default()
            }
        } else {
            Reclaimable {
                bytes: facts.alloc_size,
                ..Reclaimable::default()
            }
        });
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
        let mut entries_by_inode: HashMap<(u32, u64), Vec<NodeId>> = HashMap::new();
        for member in self.subtree(id) {
            if self.node(member).kind == NodeKind::File {
                let node = self.node(member);
                entries_by_inode
                    .entry((node.real_dev, node.file_id))
                    .or_default()
                    .push(member);
            }
        }
        let mut total = Reclaimable::default();
        for entries in entries_by_inode.values() {
            let node = self.node(entries[0]);
            if (node.link_count as usize) > entries.len() {
                total.held_by_hard_links += node.alloc_size;
                continue;
            }
            let facts = FileFacts {
                compressed: node.flags & UF_COMPRESSED != 0,
                shares_all_blocks: node.ext_flags & u64::from(EF_SHARES_ALL_BLOCKS) != 0,
                clone_refcount: node.clone_refcount,
                stream_inodes: node.shares_with + 1,
                alloc_size: node.alloc_size,
            };
            let path = self.physical_path(entries[0]);
            match reclaimable_file(facts, || private_size(&path)) {
                Ok(reclaimable) => total += reclaimable,
                Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(total)
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
