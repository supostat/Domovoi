use std::collections::HashMap;
use std::path::PathBuf;

use crate::index::Index;
use crate::installed_bundle::InstalledBundle;
use crate::node_kind::NodeKind;

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
                    None => InstalledBundle::read(&self.physical_path(id), &canonical),
                });
                continue;
            }
            pending.extend(self.children(id));
        }
        bundles.sort_by(|left, right| left.path.cmp(&right.path));
        bundles
    }
}
