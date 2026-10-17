use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrphanRow {
    pub path: String,
    pub bytes: u64,
    pub reason: String,
}
