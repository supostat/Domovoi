use std::collections::{HashMap, HashSet};

use domovoi_apfs_sys::sys::{EF_MAY_SHARE_BLOCKS, EF_SHARES_ALL_BLOCKS, UF_COMPRESSED};

use crate::badges::Badges;
use crate::index::Index;
use crate::index_node::IndexNode;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;

type InodeKey = (u32, u64);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CopyGroups {
    links: HashMap<InodeKey, Vec<NodeId>>,
    streams: HashMap<InodeKey, Vec<NodeId>>,
}

impl CopyGroups {
    fn members(groups: &HashMap<InodeKey, Vec<NodeId>>, key: InodeKey) -> Vec<NodeId> {
        groups.get(&key).cloned().unwrap_or_default()
    }

    fn contains(groups: &HashMap<InodeKey, Vec<NodeId>>, key: InodeKey, id: NodeId) -> bool {
        groups.get(&key).is_some_and(|members| members.contains(&id))
    }

    fn remove(groups: &mut HashMap<InodeKey, Vec<NodeId>>, key: InodeKey, id: NodeId) -> bool {
        let Some(members) = groups.get_mut(&key) else {
            return false;
        };
        let before = members.len();
        members.retain(|member| *member != id);
        let removed = members.len() != before;
        if members.is_empty() {
            groups.remove(&key);
        }
        removed
    }
}

fn is_link_member(node: &IndexNode) -> bool {
    node.link_count > 1 && node.kind != NodeKind::Directory
}

fn link_key(node: &IndexNode) -> InodeKey {
    (node.real_dev, node.file_id)
}

fn is_stream_candidate(node: &IndexNode) -> bool {
    node.kind == NodeKind::File
        && (node.ext_flags & u64::from(EF_MAY_SHARE_BLOCKS | EF_SHARES_ALL_BLOCKS) != 0
            || node.clone_refcount > 1
            || node.flags & UF_COMPRESSED != 0)
}

fn stream_key(node: &IndexNode) -> InodeKey {
    (node.real_dev, node.clone_id)
}

impl Index {
    pub(crate) fn collect_copy_groups(&self, live: &[NodeId]) -> CopyGroups {
        let mut groups = CopyGroups::default();
        for id in live {
            let node = self.node(*id);
            if is_link_member(node) {
                groups.links.entry(link_key(node)).or_default().push(*id);
            }
        }
        let link_canonicals: HashMap<InodeKey, NodeId> = groups
            .links
            .iter()
            .filter(|(_, members)| members.len() > 1)
            .map(|(key, members)| (*key, self.link_canonical(members)))
            .collect();
        for id in live {
            let node = self.node(*id);
            let link_counted = !is_link_member(node)
                || link_canonicals
                    .get(&link_key(node))
                    .is_none_or(|canonical| canonical == id);
            if link_counted && is_stream_candidate(node) {
                groups.streams.entry(stream_key(node)).or_default().push(*id);
            }
        }
        groups
    }

    pub(crate) fn choose_canonical_copies(&mut self, live: &[NodeId]) -> Vec<bool> {
        let groups = self.collect_copy_groups(live);
        let mut counted = vec![false; self.nodes.len()];
        for id in live {
            let node = &mut self.nodes[id.index()];
            counted[id.index()] = node.kind != NodeKind::Unknown;
            node.shares_with = 0;
            node.badges.remove(Badges::HARD_LINK);
            node.badges.remove(Badges::CLONE);
            node.badges.remove(Badges::CLONE_CANONICAL);
            if node.flags & UF_COMPRESSED != 0 {
                node.badges.insert(Badges::COMPRESSED);
            }
        }
        for members in groups.links.values().filter(|members| members.len() > 1) {
            let canonical = self.link_canonical(members);
            for member in members {
                self.nodes[member.index()].badges.insert(Badges::HARD_LINK);
                counted[member.index()] = *member == canonical;
            }
        }
        for members in groups.streams.values().filter(|members| members.len() > 1) {
            let canonical = self.stream_canonical(members);
            for member in members {
                self.mark_clone(*member, members.len(), Some(canonical));
                counted[member.index()] = *member == canonical;
            }
        }
        self.copy_groups = groups;
        counted
    }

    pub(crate) fn regroup(&mut self, removed: &[NodeId], added: &[NodeId]) -> HashMap<NodeId, bool> {
        let mut link_keys = HashSet::new();
        let mut stream_keys = HashSet::new();
        for id in removed {
            let node = &self.nodes[id.index()];
            if is_link_member(node) {
                let key = link_key(node);
                CopyGroups::remove(&mut self.copy_groups.links, key, *id);
                link_keys.insert(key);
            }
            let key = stream_key(node);
            if is_stream_candidate(node) && CopyGroups::remove(&mut self.copy_groups.streams, key, *id) {
                stream_keys.insert(key);
            }
        }
        let mut counted = HashMap::new();
        for id in added {
            let node = &mut self.nodes[id.index()];
            if node.flags & UF_COMPRESSED != 0 {
                node.badges.insert(Badges::COMPRESSED);
            }
            if node.kind == NodeKind::Directory {
                continue;
            }
            if is_link_member(node) {
                let key = link_key(node);
                self.copy_groups.links.entry(key).or_default().push(*id);
                link_keys.insert(key);
            } else {
                counted.insert(*id, node.kind != NodeKind::Unknown);
                if is_stream_candidate(node) {
                    let key = stream_key(node);
                    self.copy_groups.streams.entry(key).or_default().push(*id);
                    stream_keys.insert(key);
                }
            }
        }
        for key in link_keys {
            let members = CopyGroups::members(&self.copy_groups.links, key);
            let canonical = (members.len() > 1).then(|| self.link_canonical(&members));
            for member in members {
                let link_counted = canonical.is_none_or(|canonical| canonical == member);
                let node = &mut self.nodes[member.index()];
                if canonical.is_some() {
                    node.badges.insert(Badges::HARD_LINK);
                } else {
                    node.badges.remove(Badges::HARD_LINK);
                }
                counted.insert(member, link_counted);
                if is_stream_candidate(node) {
                    let stream = stream_key(node);
                    let present = CopyGroups::contains(&self.copy_groups.streams, stream, member);
                    if link_counted && !present {
                        self.copy_groups.streams.entry(stream).or_default().push(member);
                    } else if !link_counted && present {
                        CopyGroups::remove(&mut self.copy_groups.streams, stream, member);
                        self.mark_clone(member, 1, None);
                    }
                    stream_keys.insert(stream);
                }
            }
        }
        for key in stream_keys {
            let members = CopyGroups::members(&self.copy_groups.streams, key);
            let canonical = (members.len() > 1).then(|| self.stream_canonical(&members));
            for member in &members {
                self.mark_clone(*member, members.len(), canonical);
                counted.insert(*member, canonical.is_none_or(|canonical| canonical == *member));
            }
        }
        counted
    }

    fn mark_clone(&mut self, id: NodeId, group_size: usize, canonical: Option<NodeId>) {
        let node = &mut self.nodes[id.index()];
        node.shares_with = u32::try_from(group_size - 1).expect("clone group size fits in u32");
        node.badges.remove(Badges::CLONE);
        node.badges.remove(Badges::CLONE_CANONICAL);
        match canonical {
            Some(canonical) if canonical == id => node.badges.insert(Badges::CLONE_CANONICAL),
            Some(_) => node.badges.insert(Badges::CLONE),
            None => {}
        }
    }

    fn link_canonical(&self, members: &[NodeId]) -> NodeId {
        *members
            .iter()
            .min_by_key(|member| self.relative_path(**member))
            .expect("a hard-link group is not empty")
    }

    fn stream_canonical(&self, members: &[NodeId]) -> NodeId {
        *members
            .iter()
            .min_by_key(|member| {
                let node = self.node(**member);
                (self.inside_dependency_store(**member), node.real_dev, node.file_id)
            })
            .expect("a clone group is not empty")
    }
}
