use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::index::Index;
use crate::installed_bundle::{INFO_PLIST, InstalledBundle};
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::reach::Reach;

impl Index {
    pub(crate) fn discover_bundles(&self, previous: &[InstalledBundle], changed: &[PathBuf]) -> Vec<InstalledBundle> {
        let reusable: HashMap<&str, &InstalledBundle> = previous
            .iter()
            .filter(|bundle| !changed.iter().any(|path| bundle.contains(path)))
            .map(|bundle| (bundle.path.as_str(), bundle))
            .collect();
        let mut bundles = Vec::new();
        let mut pending = vec![self.root()];
        while let Some(id) = pending.pop() {
            if self.node(id).kind == NodeKind::Directory && InstalledBundle::is_bundle_name(self.name(id)) {
                let canonical = self.path(id);
                let known = canonical.to_str().and_then(|path| reusable.get(path));
                bundles.extend(match known {
                    Some(bundle) => Some((*bundle).clone()),
                    None if self.holds_info_plist_on_disk(id) => {
                        InstalledBundle::read(&self.physical_path(id), &canonical)
                    }
                    None => None,
                });
                continue;
            }
            pending.extend(self.children(id));
        }
        bundles.sort_by(|left, right| left.path.cmp(&right.path));
        bundles
    }

    fn holds_info_plist_on_disk(&self, bundle: NodeId) -> bool {
        Path::new(INFO_PLIST)
            .iter()
            .try_fold(bundle, |node, name| self.child_named(node, name))
            .is_some_and(|info_plist| self.node(info_plist).reach == Reach::Leaf)
    }
}
