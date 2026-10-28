use std::collections::BTreeSet;
use std::ffi::OsStr;

use crate::container_metadata::container_identifier;
use crate::group_container::group_owner;
use crate::index::Index;
use crate::launchd_job::launchd_owner;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::owner_evidence::OwnerEvidence;
use crate::path_rules::AppDataNaming;

const PLIST_SUFFIX: &str = ".plist";

impl Index {
    pub(crate) fn claimed_owner(
        &self,
        id: NodeId,
        name: &OsStr,
        naming: AppDataNaming,
    ) -> Option<(String, OwnerEvidence)> {
        let name = name.to_str()?;
        match naming {
            AppDataNaming::BundleId => bundle_id_named(name),
            AppDataNaming::BundleIdOrName => bundle_id_named(name).or_else(|| self.named_bundle(name)),
            AppDataNaming::Suffixed(suffix) => name.strip_suffix(suffix).and_then(bundle_id_named),
            AppDataNaming::Container => container_identifier(&self.physical_path(id))
                .filter(|identifier| looks_like_bundle_id(identifier))
                .map(|identifier| (identifier, OwnerEvidence::ContainerMetadata))
                .or_else(|| bundle_id_named(name)),
            AppDataNaming::GroupContainer => group_owner(name, &self.bundles),
            AppDataNaming::LaunchdPlist if self.node(id).kind != NodeKind::Directory => {
                launchd_owner(&self.physical_path(id), &self.bundles)
                    .or_else(|| name.strip_suffix(PLIST_SUFFIX).and_then(bundle_id_named))
            }
            AppDataNaming::LaunchdPlist => None,
        }
    }

    fn named_bundle(&self, name: &str) -> Option<(String, OwnerEvidence)> {
        let folded = name.to_lowercase();
        let owners: BTreeSet<&str> = self
            .bundles
            .iter()
            .filter(|bundle| bundle.name.to_lowercase() == folded)
            .map(|bundle| bundle.bundle_id.as_str())
            .collect();
        let mut owners = owners.into_iter();
        let owner = owners.next()?;
        owners
            .next()
            .is_none()
            .then(|| (owner.to_owned(), OwnerEvidence::NameOnly))
    }
}

pub(crate) fn bundle_id_named(candidate: &str) -> Option<(String, OwnerEvidence)> {
    looks_like_bundle_id(candidate).then(|| (candidate.to_owned(), OwnerEvidence::BundleIdName))
}

pub(crate) fn looks_like_bundle_id(candidate: &str) -> bool {
    candidate.split('.').count() >= 2
        && candidate.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
}
