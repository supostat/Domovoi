use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::io;
use std::ops::AddAssign;

use domovoi_apfs_sys::private_size;
use domovoi_apfs_sys::sys::{EF_SHARES_ALL_BLOCKS, UF_COMPRESSED};
use domovoi_scanner::Node;

use crate::copy_groups::{InodeKey, is_link_member, is_stream_candidate, link_key, stream_key};
use crate::index::{Index, record};
use crate::index_node::IndexNode;
use crate::name_table::NameId;
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
    Ok(split_private(facts, private_size()?))
}

fn split_private(facts: FileFacts, private_size: u64) -> Reclaimable {
    let private = private_size.min(facts.alloc_size);
    let held = facts.alloc_size - private;
    let held_by_clone = facts.clone_refcount > 1 || facts.shares_all_blocks;
    Reclaimable {
        bytes: private,
        held_by_clones: if held_by_clone { held } else { 0 },
        held_by_snapshots: if held_by_clone { 0 } else { held },
        held_by_hard_links: 0,
    }
}

impl FileFacts {
    fn of(node: &IndexNode, stream_inodes: u32) -> Self {
        Self {
            compressed: node.flags & UF_COMPRESSED != 0,
            shares_all_blocks: node.ext_flags & u64::from(EF_SHARES_ALL_BLOCKS) != 0,
            clone_refcount: node.clone_refcount,
            stream_inodes,
            alloc_size: node.alloc_size,
        }
    }
}

pub fn measure_reclaimable(root: &Node) -> Reclaimable {
    let files = scanned_files(root);
    let mut linked = HashSet::new();
    let mut streams: HashMap<InodeKey, u32> = HashMap::new();
    for (file, _) in &files {
        let counted = !is_link_member(file) || linked.insert(link_key(file));
        if counted && is_stream_candidate(file) {
            *streams.entry(stream_key(file)).or_default() += 1;
        }
    }
    let Ok(total) = sum_over_files(files.iter().map(|(file, private)| (file, *private)), |file, private| {
        let stream_inodes = if is_stream_candidate(file) {
            streams[&stream_key(file)]
        } else {
            1
        };
        Ok::<_, Infallible>(measured_file(FileFacts::of(file, stream_inodes), private))
    });
    total
}

fn scanned_files(root: &Node) -> Vec<(IndexNode, Option<u64>)> {
    let mut files = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let file = record(node, NodeId::NONE, NameId::NONE);
        if file.kind == NodeKind::File {
            files.push((
                file,
                node.attributes.as_ref().and_then(|attributes| attributes.private_size),
            ));
        }
        pending.extend(&node.children);
    }
    files
}

fn measured_file(facts: FileFacts, private_size: Option<u64>) -> Reclaimable {
    match private_size {
        Some(private) if !facts.compressed => split_private(facts, private),
        _ => estimate_file(facts),
    }
}

fn sum_over_files<'file, T, E>(
    files: impl IntoIterator<Item = (&'file IndexNode, T)>,
    mut reclaim: impl FnMut(&'file IndexNode, T) -> Result<Reclaimable, E>,
) -> Result<Reclaimable, E> {
    let mut total = Reclaimable::default();
    let mut linked: HashMap<InodeKey, (usize, &'file IndexNode, T)> = HashMap::new();
    for (file, item) in files {
        if file.link_count > 1 {
            linked
                .entry(link_key(file))
                .and_modify(|group| group.0 += 1)
                .or_insert((1, file, item));
        } else {
            total += reclaim(file, item)?;
        }
    }
    for (seen, file, item) in linked.into_values() {
        if file.link_count as usize > seen {
            total.held_by_hard_links += file.alloc_size;
        } else {
            total += reclaim(file, item)?;
        }
    }
    Ok(total)
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
        let files = self
            .subtree(id)
            .into_iter()
            .filter(|member| self.node(*member).kind == NodeKind::File)
            .map(|member| (self.node(member), member));
        sum_over_files(files, |file, member| {
            reclaim(member, FileFacts::of(file, file.shares_with + 1))
        })
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
