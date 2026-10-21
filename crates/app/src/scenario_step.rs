use std::path::PathBuf;

use serde::Deserialize;

use crate::safety_badge::SafetyBadge;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenarioStep {
    Scan,
    ReopenCache,
    Rescan {
        path: PathBuf,
    },
    CreateFile {
        path: PathBuf,
        bytes: usize,
    },
    AwaitChange {
        path: PathBuf,
        timeout_seconds: u64,
        #[serde(default)]
        heavy_nodes: Option<usize>,
    },
    Recommendations,
    TrashRecommendations {
        level: SafetyBadge,
    },
    RestoreLast,
    FindDuplicates,
    ReplaceWithClone,
    PurgeTrash,
    DownloadModel {
        catalog: PathBuf,
        entry: String,
        mirror: String,
        models: PathBuf,
    },
    AskAssistant {
        question: String,
        #[serde(default)]
        max_tokens_per_step: Option<u32>,
    },
    ScrollDiskList {
        path: PathBuf,
    },
    UpdateDiskScreen,
    Snapshots {
        directory: PathBuf,
        scale: f64,
    },
}
