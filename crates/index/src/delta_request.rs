use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::changed_path::ChangedPath;
use crate::delta_target::DeltaTarget;
use crate::index::Index;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::reach::Reach;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeltaRequest {
    pub(crate) generation: u64,
    pub(crate) targets: Vec<DeltaTarget>,
}

impl DeltaRequest {
    pub fn targets(&self) -> &[DeltaTarget] {
        &self.targets
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

impl Index {
    pub fn delta_request(&self, changed: &[ChangedPath]) -> DeltaRequest {
        let mut subtree_by_path: BTreeMap<PathBuf, (NodeId, bool)> = BTreeMap::new();
        for change in changed {
            let Some((directory, path)) = self.nearest_listed_directory(&change.path) else {
                continue;
            };
            let subtree = change.subtree && path == change.path;
            subtree_by_path
                .entry(path)
                .and_modify(|(_, existing)| *existing |= subtree)
                .or_insert((directory, subtree));
        }
        let subtree_paths: Vec<PathBuf> = subtree_by_path
            .iter()
            .filter(|(_, (_, subtree))| *subtree)
            .map(|(path, _)| path.clone())
            .collect();
        let targets = subtree_by_path
            .into_iter()
            .filter(|(path, _)| {
                !subtree_paths
                    .iter()
                    .any(|outer| path != outer && path.starts_with(outer))
            })
            .map(|(path, (directory, subtree))| DeltaTarget {
                directory,
                physical: self.physical_path(directory),
                known_directories: self.listed_child_directories(directory),
                path,
                subtree,
            })
            .collect();
        DeltaRequest {
            generation: self.generation,
            targets,
        }
    }

    fn nearest_listed_directory(&self, changed: &Path) -> Option<(NodeId, PathBuf)> {
        let mut candidate = changed.to_path_buf();
        loop {
            if let Some(node) = self.find(&candidate)
                && self.is_listed_directory(node)
                && fs::symlink_metadata(self.physical_path(node)).is_ok_and(|metadata| metadata.is_dir())
            {
                return Some((node, candidate));
            }
            if !candidate.pop() {
                return None;
            }
        }
    }

    fn is_listed_directory(&self, id: NodeId) -> bool {
        let node = self.node(id);
        node.reach == Reach::Listed && (id == self.root() || node.kind == NodeKind::Directory)
    }

    fn listed_child_directories(&self, id: NodeId) -> Vec<(OsString, u64)> {
        self.children(id)
            .filter(|child| self.is_listed_directory(*child))
            .map(|child| (self.name(child).to_owned(), self.node(child).file_id))
            .collect()
    }
}
