use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use domovoi_scanner::Node;

use crate::copy_groups::{InodeKey, is_link_member, is_stream_candidate, link_key, stream_key};
use crate::index::record;
use crate::index_node::IndexNode;
use crate::name_table::NameId;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::reclaimable::{FileFacts, FileReading, Reclaimable, reclaim_alone, tally_files};

#[derive(Clone, Debug, Default)]
pub struct MeasuredSet {
    alone: Reclaimable,
    shared: Vec<(IndexNode, Option<u64>)>,
}

impl MeasuredSet {
    pub fn of(root: &Node) -> Self {
        let mut set = Self::default();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            let file = record(node, NodeId::NONE, NameId::NONE);
            if file.kind == NodeKind::File {
                let private = node.attributes.as_ref().and_then(|attributes| attributes.private_size);
                if is_link_member(&file) || is_stream_candidate(&file) {
                    set.shared.push((file, private));
                } else {
                    let facts = FileFacts::of(&file, 1);
                    set.alone += reclaim_alone(facts, reading(facts, private));
                }
            }
            pending.extend(&node.children);
        }
        set
    }

    pub fn absorb(&mut self, other: Self) {
        self.alone += other.alone;
        self.shared.extend(other.shared);
    }

    pub fn reclaimable(&self) -> Reclaimable {
        let mut linked = HashSet::new();
        let mut streams: HashMap<InodeKey, u32> = HashMap::new();
        for (file, _) in &self.shared {
            let counted = !is_link_member(file) || linked.insert(link_key(file));
            if counted && is_stream_candidate(file) {
                *streams.entry(stream_key(file)).or_default() += 1;
            }
        }
        let stream_inodes = |file: &IndexNode| {
            if is_stream_candidate(file) {
                streams[&stream_key(file)]
            } else {
                1
            }
        };
        let Ok(tally) = tally_files(
            self.shared.iter().map(|(file, private)| (file, *private)),
            stream_inodes,
            |facts, private| Ok::<_, Infallible>(reading(facts, private)),
        );
        let mut total = self.alone;
        total += tally.total;
        total
    }
}

pub fn measure_reclaimable(root: &Node) -> Reclaimable {
    MeasuredSet::of(root).reclaimable()
}

pub fn measure_reclaimable_of(roots: &[&Node]) -> Reclaimable {
    let mut set = MeasuredSet::default();
    for root in roots {
        set.absorb(MeasuredSet::of(root));
    }
    set.reclaimable()
}

fn reading(facts: FileFacts, private: Option<u64>) -> FileReading {
    match private {
        Some(size) if !facts.compressed => FileReading::Private(size),
        _ => FileReading::Unread,
    }
}
