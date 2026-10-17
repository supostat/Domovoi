use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrayOutcome {
    pub moved: u64,
    #[serde(default)]
    pub failures: Vec<String>,
}
