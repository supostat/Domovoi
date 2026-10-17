use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelCardState {
    Available,
    Downloading { received: u64, bytes_per_second: u64 },
    Paused { received: u64 },
    Failed { reason: String },
    Checking,
    Installed,
    Active,
}
