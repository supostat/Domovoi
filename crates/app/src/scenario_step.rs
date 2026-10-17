use std::path::PathBuf;

use serde::Deserialize;

use crate::safety_badge::SafetyBadge;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenarioStep {
    Scan,
    ReopenCache,
    AwaitChange { path: PathBuf, timeout_seconds: u64 },
    Recommendations,
    TrashRecommendations { level: SafetyBadge },
    RestoreLast,
    FindDuplicates,
    ReplaceWithClone,
    Snapshots { directory: PathBuf, scale: f64 },
}
