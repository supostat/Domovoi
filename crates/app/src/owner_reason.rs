use domovoi_index::{Index, Installation, NodeId, OwnerEvidence};
use domovoi_rules::RemovalReason;

use crate::strings::{
    OWNER, OWNER_APP_GROUP, OWNER_BUNDLE_ID_NAME, OWNER_CONTAINER_METADATA, OWNER_LAUNCHD, OWNER_NAME_ONLY,
    OWNER_REMOVED, OWNER_TEAM_PREFIX, REMOVAL_BUNDLE,
};

pub(crate) fn evidence_phrase(evidence: OwnerEvidence) -> &'static str {
    match evidence {
        OwnerEvidence::BundleIdName => OWNER_BUNDLE_ID_NAME,
        OwnerEvidence::ContainerMetadata => OWNER_CONTAINER_METADATA,
        OwnerEvidence::AppGroup => OWNER_APP_GROUP,
        OwnerEvidence::TeamPrefix => OWNER_TEAM_PREFIX,
        OwnerEvidence::LaunchdAssociation => OWNER_LAUNCHD,
        OwnerEvidence::NameOnly => OWNER_NAME_ONLY,
    }
}

pub(crate) fn removal_phrase(reason: RemovalReason) -> &'static str {
    match reason {
        RemovalReason::Bundle => REMOVAL_BUNDLE,
        RemovalReason::Owner(evidence) => evidence_phrase(evidence),
    }
}

pub(crate) fn owner_line(index: &Index, node: NodeId) -> Option<String> {
    let ownership = index.ownership(node)?;
    let app = index.app(ownership.app)?;
    let evidence = evidence_phrase(ownership.evidence);
    Some(match &app.installation {
        Installation::Installed { bundle_path } => {
            let name = index
                .bundles()
                .iter()
                .find(|bundle| &bundle.path == bundle_path)
                .map_or(app.bundle_id.as_str(), |bundle| bundle.name.as_str());
            format!("{OWNER} {name} · {evidence}")
        }
        Installation::Missing => format!("{OWNER} {} · {OWNER_REMOVED} · {evidence}", app.bundle_id),
        Installation::System => format!("{OWNER} {} · {evidence}", app.bundle_id),
    })
}
