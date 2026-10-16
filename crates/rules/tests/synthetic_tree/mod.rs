use std::ffi::OsString;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use domovoi_apfs_sys::{EntryAttributes, ObjectKind};
use domovoi_index::{Enrichment, FirmlinkPair, Index, IndexRoot};
use domovoi_scanner::{Node, NodeState, ScanReport};

static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(1_000);

fn node(name: &str, kind: ObjectKind, children: Vec<Node>) -> Node {
    let file_id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let file = kind != ObjectKind::Directory;
    Node {
        name: OsString::from(name),
        attributes: Some(EntryAttributes {
            kind,
            flags: 0,
            file_id,
            created: time,
            modified: time,
            added: Some(time),
            mount_status: (!file).then_some(0),
            link_count: file.then_some(1),
            total_size: file.then_some(4096),
            alloc_size: file.then_some(4096),
            data_alloc_size: file.then_some(4096),
            private_size: None,
            link_id: Some(file_id),
            real_dev: Some(1),
            clone_id: Some(file_id),
            ext_flags: Some(0),
            clone_refcount: file.then_some(1),
        }),
        last_used: None,
        state: if file { NodeState::Leaf } else { NodeState::Listed },
        children,
    }
}

pub(crate) fn directory(name: &str, children: Vec<Node>) -> Node {
    node(name, ObjectKind::Directory, children)
}

pub(crate) fn file(name: &str) -> Node {
    node(name, ObjectKind::File, Vec::new())
}

pub(crate) fn path_tree(path: &str, leaf: Node) -> Node {
    let mut parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let mut current = leaf;
    while let Some(name) = parts.pop() {
        current = directory(name, vec![current]);
    }
    current
}

pub(crate) fn data_volume_index(children: Vec<Node>) -> Index {
    let pair = |path: &str, data_relative: &str| FirmlinkPair {
        path: path.to_owned(),
        data_relative: data_relative.to_owned(),
    };
    let report = ScanReport {
        root: Node {
            name: OsString::from("/System/Volumes/Data"),
            attributes: None,
            last_used: None,
            state: NodeState::Listed,
            children,
        },
        files: 0,
        bytes: 0,
    };
    let root = IndexRoot::DataVolume {
        firmlinks: vec![
            pair("/Users", "Users"),
            pair("/Library", "Library"),
            pair("/Applications", "Applications"),
        ],
    };
    Index::build(&report, root, &Enrichment::default())
}

pub(crate) fn merge(trees: Vec<Node>) -> Vec<Node> {
    let mut merged: Vec<Node> = Vec::new();
    for tree in trees {
        match merged.iter_mut().find(|existing| existing.name == tree.name) {
            Some(existing) => {
                let mut children = std::mem::take(&mut existing.children);
                children.extend(tree.children);
                existing.children = merge(children);
            }
            None => merged.push(tree),
        }
    }
    merged
}
