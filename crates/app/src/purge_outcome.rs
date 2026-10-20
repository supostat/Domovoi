use serde::Deserialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurgeOutcome {
    pub removed: usize,
    #[serde(default)]
    pub failures: Vec<String>,
}
