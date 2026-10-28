use domovoi_index::{Index, Installation, NodeId, OwnerEvidence};
use serde_json::{Value, json};

pub(crate) fn owner_record(index: &Index, node: NodeId) -> Value {
    let Some(ownership) = index.ownership(node) else {
        return Value::Null;
    };
    let Some(app) = index.app(ownership.app) else {
        return Value::Null;
    };
    json!({
        "bundle_id": app.bundle_id,
        "installation": installation_name(&app.installation),
        "evidence": evidence_name(ownership.evidence),
    })
}

fn installation_name(installation: &Installation) -> &'static str {
    match installation {
        Installation::Installed { .. } => "installed",
        Installation::System => "system",
        Installation::Missing => "missing",
    }
}

fn evidence_name(evidence: OwnerEvidence) -> &'static str {
    match evidence {
        OwnerEvidence::BundleIdName => "bundle_id_name",
        OwnerEvidence::ContainerMetadata => "container_metadata",
        OwnerEvidence::AppGroup => "app_group",
        OwnerEvidence::TeamPrefix => "team_prefix",
        OwnerEvidence::LaunchdAssociation => "launchd_association",
        OwnerEvidence::NameOnly => "name_only",
    }
}
