use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Component, Path};

use domovoi_index::{Index, NodeId, Reclaimable, Timestamp};

use crate::blocked_item::BlockedItem;
use crate::claim_ledger::ancestry;
use crate::project_item::ProjectItem;

const REPOSITORY: &str = ".git";
const USERS: &str = "Users";
const HOME_DEPTH: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectCard {
    pub root: NodeId,
    pub items: Vec<ProjectItem>,
    pub blocked: Vec<BlockedItem>,
    pub reclaimable: Reclaimable,
    pub last_work: Timestamp,
}

impl ProjectCard {
    pub(crate) fn empty(root: NodeId) -> Self {
        Self {
            root,
            items: Vec::new(),
            blocked: Vec::new(),
            reclaimable: Reclaimable::default(),
            last_work: Timestamp::UNKNOWN,
        }
    }

    pub(crate) fn root_of(index: &Index, marker_folder: NodeId) -> NodeId {
        ancestry(index, marker_folder)
            .take_while(|folder| *folder != index.root() && !is_home(&index.path(*folder)))
            .find(|folder| index.child_named(*folder, OsStr::new(REPOSITORY)).is_some())
            .unwrap_or(marker_folder)
    }

    pub(crate) fn settle_last_work(&mut self, index: &Index) {
        let artifacts: HashSet<NodeId> = self
            .items
            .iter()
            .map(|item| item.node)
            .chain(self.blocked.iter().map(|blocked| blocked.node))
            .collect();
        let on_the_way: HashSet<NodeId> = artifacts
            .iter()
            .flat_map(|artifact| {
                ancestry(index, *artifact)
                    .skip(1)
                    .take_while(|folder| *folder != self.root)
            })
            .collect();
        self.last_work = newest_outside(index, self.root, &artifacts, &on_the_way);
    }
}

fn newest_outside(
    index: &Index,
    folder: NodeId,
    artifacts: &HashSet<NodeId>,
    on_the_way: &HashSet<NodeId>,
) -> Timestamp {
    index
        .children(folder)
        .filter(|child| !artifacts.contains(child) && index.name(*child) != OsStr::new(REPOSITORY))
        .map(|child| {
            if on_the_way.contains(&child) {
                newest_outside(index, child, artifacts, on_the_way)
            } else {
                index.node(child).newest_modified
            }
        })
        .max()
        .unwrap_or(Timestamp::UNKNOWN)
}

fn is_home(path: &Path) -> bool {
    let names: Vec<&OsStr> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    names.len() == HOME_DEPTH && names[0] == OsStr::new(USERS)
}
