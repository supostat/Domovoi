use std::collections::{BTreeSet, HashMap};
use std::convert::Infallible;
use std::io;
use std::ops::AddAssign;

use domovoi_apfs_sys::private_size;
use domovoi_apfs_sys::sys::{EF_SHARES_ALL_BLOCKS, UF_COMPRESSED};

use crate::copy_groups::{InodeKey, link_key, stream_key};
use crate::held_group::{HeldGroup, HeldKey};
use crate::index::Index;
use crate::index_node::IndexNode;
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

impl FileFacts {
    pub(crate) fn of(node: &IndexNode, stream_inodes: u32) -> Self {
        Self {
            compressed: node.flags & UF_COMPRESSED != 0,
            shares_all_blocks: node.ext_flags & u64::from(EF_SHARES_ALL_BLOCKS) != 0,
            clone_refcount: node.clone_refcount,
            stream_inodes,
            alloc_size: node.alloc_size,
        }
    }

    fn is_regular_clone(self) -> bool {
        !self.compressed && self.clone_refcount > 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileReading {
    Vanished,
    Unread,
    Private(u64),
}

pub(crate) fn estimate_file(facts: FileFacts) -> Reclaimable {
    if facts.shares_all_blocks || facts.stream_inodes > 1 || facts.is_regular_clone() {
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

fn read_file(facts: FileFacts, private_size: impl FnOnce() -> io::Result<u64>) -> io::Result<FileReading> {
    if facts.compressed {
        return Ok(FileReading::Unread);
    }
    match private_size() {
        Ok(size) => Ok(FileReading::Private(size)),
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => Ok(FileReading::Vanished),
        Err(error) if error.kind() == io::ErrorKind::Unsupported => Ok(FileReading::Unread),
        Err(error) => Err(error),
    }
}

pub(crate) fn reclaim_alone(facts: FileFacts, reading: FileReading) -> Reclaimable {
    match reading {
        FileReading::Vanished => Reclaimable::default(),
        FileReading::Unread => estimate_file(facts),
        FileReading::Private(size) => split_private(facts, size),
    }
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

fn reclaim_together(members: &[(FileFacts, FileReading)]) -> Reclaimable {
    let mut private = 0;
    let mut shared = 0;
    for (facts, reading) in members {
        let own = match reading {
            FileReading::Vanished => continue,
            FileReading::Unread => 0,
            FileReading::Private(size) => (*size).min(facts.alloc_size),
        };
        private += own;
        shared = shared.max(facts.alloc_size - own);
    }
    Reclaimable {
        bytes: private + shared,
        ..Reclaimable::default()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Tally {
    pub(crate) total: Reclaimable,
    held: Vec<HeldGroup>,
}

pub(crate) fn tally_files<'file, T, E>(
    files: impl IntoIterator<Item = (&'file IndexNode, T)>,
    stream_inodes: impl Fn(&IndexNode) -> u32,
    mut read: impl FnMut(FileFacts, T) -> Result<FileReading, E>,
) -> Result<Tally, E> {
    let mut tally = Tally::default();
    let mut streams: HashMap<InodeKey, Vec<(FileFacts, T)>> = HashMap::new();
    for (file, item) in freed_inodes(files, &mut tally) {
        let facts = FileFacts::of(file, stream_inodes(file));
        if facts.is_regular_clone() {
            streams.entry(stream_key(file)).or_default().push((facts, item));
        } else {
            tally.total += reclaim_alone(facts, read(facts, item)?);
        }
    }
    for (key, members) in streams {
        settle_stream(key, members, &mut read, &mut tally)?;
    }
    tally.held.sort_unstable_by_key(|group| group.key);
    Ok(tally)
}

fn freed_inodes<'file, T>(
    files: impl IntoIterator<Item = (&'file IndexNode, T)>,
    tally: &mut Tally,
) -> Vec<(&'file IndexNode, T)> {
    let mut linked: HashMap<InodeKey, (u32, &'file IndexNode, T)> = HashMap::new();
    let mut inodes = Vec::new();
    for (file, item) in files {
        if file.link_count > 1 {
            linked
                .entry(link_key(file))
                .and_modify(|group| group.0 += 1)
                .or_insert((1, file, item));
        } else {
            inodes.push((file, item));
        }
    }
    for (key, (inside, file, item)) in linked {
        if inside >= file.link_count {
            inodes.push((file, item));
        } else {
            tally.total.held_by_hard_links += file.alloc_size;
            tally.held.push(HeldGroup {
                key: HeldKey::hard_links(key),
                bytes: file.alloc_size,
                members_inside: inside,
                members_total: file.link_count,
            });
        }
    }
    inodes
}

fn settle_stream<T, E>(
    key: InodeKey,
    members: Vec<(FileFacts, T)>,
    read: &mut impl FnMut(FileFacts, T) -> Result<FileReading, E>,
    tally: &mut Tally,
) -> Result<(), E> {
    let members_inside = u32::try_from(members.len()).expect("a clone stream has fewer than 2^32 inodes");
    let members_total = members
        .iter()
        .map(|(facts, _)| facts.clone_refcount)
        .max()
        .expect("a clone stream is not empty");
    let bytes = members
        .iter()
        .map(|(facts, _)| facts.alloc_size)
        .max()
        .expect("a clone stream is not empty");
    let readings = members
        .into_iter()
        .map(|(facts, item)| Ok((facts, read(facts, item)?)))
        .collect::<Result<Vec<_>, E>>()?;
    if members_inside >= members_total {
        tally.total += reclaim_together(&readings);
        return Ok(());
    }
    for (facts, reading) in readings {
        tally.total += reclaim_alone(facts, reading);
    }
    tally.held.push(HeldGroup {
        key: HeldKey::clones(key),
        bytes,
        members_inside,
        members_total,
    });
    Ok(())
}

impl Index {
    pub fn reclaimable(&self, id: NodeId) -> io::Result<Reclaimable> {
        self.reclaimable_of(&[id])
    }

    pub fn reclaimable_of(&self, ids: &[NodeId]) -> io::Result<Reclaimable> {
        let tally = self.tally(ids, |member, facts| {
            read_file(facts, || private_size(&self.physical_path(member)))
        })?;
        Ok(tally.total)
    }

    pub fn reclaimable_estimate(&self, id: NodeId) -> Reclaimable {
        self.reclaimable_estimate_of(&[id])
    }

    pub fn reclaimable_estimate_of(&self, ids: &[NodeId]) -> Reclaimable {
        self.estimate_tally(ids).total
    }

    pub fn reclaimable_estimate_with_held_groups(&self, id: NodeId) -> (Reclaimable, Vec<HeldGroup>) {
        let tally = self.estimate_tally(&[id]);
        (tally.total, tally.held)
    }

    fn estimate_tally(&self, ids: &[NodeId]) -> Tally {
        let Ok(tally) = self.tally(ids, |_, _| Ok::<_, Infallible>(FileReading::Unread));
        tally
    }

    fn tally<E>(
        &self,
        ids: &[NodeId],
        mut read: impl FnMut(NodeId, FileFacts) -> Result<FileReading, E>,
    ) -> Result<Tally, E> {
        let files = self
            .outermost(ids)
            .into_iter()
            .flat_map(|root| self.subtree(root))
            .filter(|member| self.node(*member).kind == NodeKind::File)
            .map(|member| (self.node(member), member));
        tally_files(files, |file| file.shares_with + 1, |facts, member| read(member, facts))
    }

    fn outermost(&self, ids: &[NodeId]) -> BTreeSet<NodeId> {
        let chosen: BTreeSet<NodeId> = ids.iter().copied().collect();
        chosen
            .iter()
            .copied()
            .filter(|id| !self.ancestors(*id).any(|ancestor| chosen.contains(&ancestor)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{FileFacts, FileReading, Reclaimable, read_file, reclaim_alone};

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
        let reading = read_file(facts(), || Ok(0)).expect("facts are complete");
        assert_eq!(
            reclaim_alone(facts(), reading),
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
        let reading = read_file(compressed, unread).expect("facts are complete");
        assert_eq!(reading, FileReading::Unread);
        assert_eq!(reclaim_alone(compressed, reading).bytes, ALLOCATED);
        let cloned = FileFacts {
            stream_inodes: 2,
            ..compressed
        };
        assert_eq!(reclaim_alone(cloned, reading).held_by_clones, ALLOCATED);
    }
}
