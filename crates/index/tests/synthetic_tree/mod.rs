use std::ffi::OsString;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domovoi_apfs_sys::{EntryAttributes, ObjectKind};
use domovoi_index::{FirmlinkPair, IndexRoot};
use domovoi_scanner::{Node, NodeState, ScanReport};

const BLOCK: u64 = 4096;
const CREATED: u64 = 1_700_000_000;

static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(100);

fn attributes(kind: ObjectKind, alloc_size: u64) -> EntryAttributes {
    let file_id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let created = UNIX_EPOCH + Duration::from_secs(CREATED);
    EntryAttributes {
        kind,
        flags: 0,
        file_id,
        created,
        modified: created,
        added: Some(created),
        mount_status: (kind == ObjectKind::Directory).then_some(0),
        link_count: (kind != ObjectKind::Directory).then_some(1),
        total_size: (kind != ObjectKind::Directory).then_some(alloc_size),
        alloc_size: (kind != ObjectKind::Directory).then_some(alloc_size),
        data_alloc_size: (kind != ObjectKind::Directory).then_some(alloc_size),
        private_size: None,
        link_id: Some(file_id),
        real_dev: Some(1),
        clone_id: Some(file_id),
        ext_flags: Some(0),
        clone_refcount: (kind != ObjectKind::Directory).then_some(1),
    }
}

pub(crate) fn directory(name: &str, children: Vec<Node>) -> Node {
    Node {
        name: OsString::from(name),
        attributes: Some(attributes(ObjectKind::Directory, 0)),
        last_used: None,
        state: NodeState::Listed,
        children,
    }
}

pub(crate) fn file(name: &str, blocks: u64) -> Node {
    Node {
        name: OsString::from(name),
        attributes: Some(attributes(ObjectKind::File, blocks * BLOCK)),
        last_used: None,
        state: NodeState::Leaf,
        children: Vec::new(),
    }
}

pub(crate) fn opened_file(name: &str, blocks: u64, opened: SystemTime) -> Node {
    Node {
        last_used: Some(opened),
        ..file(name, blocks)
    }
}

pub(crate) fn data_volume(children: Vec<Node>) -> (ScanReport, IndexRoot) {
    let pair = |path: &str, data_relative: &str| FirmlinkPair {
        path: path.to_owned(),
        data_relative: data_relative.to_owned(),
    };
    let root = Node {
        name: OsString::from("/System/Volumes/Data"),
        attributes: None,
        last_used: None,
        state: NodeState::Listed,
        children,
    };
    let report = ScanReport {
        root,
        files: 0,
        bytes: 0,
    };
    let firmlinks = vec![
        pair("/Users", "Users"),
        pair("/Applications", "Applications"),
        pair("/Library", "Library"),
        pair("/private", "private"),
    ];
    (report, IndexRoot::DataVolume { firmlinks })
}
