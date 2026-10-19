use std::collections::BTreeMap;
use std::path::PathBuf;

use domovoi_index::{Badges, Category, Index, NodeId, NodeKind, Reach, Timestamp};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Summary {
    kind: NodeKind,
    reach: Reach,
    category: Category,
    badges: Badges,
    owner: Option<String>,
    file_id: u64,
    allocated: u64,
    files: u32,
    flags_in_subtree: u32,
    shares_with: u32,
    newest_modified: Timestamp,
    newest_opened: Timestamp,
    last_opened: Timestamp,
    own: Option<FileFacts>,
}

#[derive(Debug, PartialEq, Eq)]
struct FileFacts {
    flags: u32,
    link_count: u32,
    clone_id: u64,
    alloc_size: u64,
    total_size: u64,
    modified: Timestamp,
    created: Timestamp,
    added: Timestamp,
}

fn summary(index: &Index, id: NodeId) -> Summary {
    let node = index.node(id);
    let own = (node.kind != NodeKind::Directory && id != index.root()).then_some(FileFacts {
        flags: node.flags,
        link_count: node.link_count,
        clone_id: node.clone_id,
        alloc_size: node.alloc_size,
        total_size: node.total_size,
        modified: node.modified,
        created: node.created,
        added: node.added,
    });
    Summary {
        kind: node.kind,
        reach: node.reach,
        category: node.category,
        badges: node.badges,
        owner: index.app(node.app).map(|app| app.bundle_id.clone()),
        file_id: node.file_id,
        allocated: node.allocated,
        files: node.files,
        flags_in_subtree: node.flags_in_subtree,
        shares_with: node.shares_with,
        newest_modified: node.newest_modified,
        newest_opened: node.newest_opened,
        last_opened: node.last_opened,
        own,
    }
}

pub(crate) fn dump(index: &Index) -> BTreeMap<PathBuf, Summary> {
    let mut pending = vec![index.root()];
    let mut nodes = BTreeMap::new();
    while let Some(id) = pending.pop() {
        nodes.insert(index.path(id), summary(index, id));
        pending.extend(index.children(id));
    }
    nodes
}
