use std::path::PathBuf;

use serde::Deserialize;

use crate::provenance::Provenance;
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
    CheckProjects {
        projects: Vec<String>,
    },
    DeleteRecommendations {
        level: SafetyBadge,
    },
    RemoveApplication {
        path: PathBuf,
    },
    MarkFromList {
        paths: Vec<PathBuf>,
    },
    UnmarkTrayRow {
        row: usize,
    },
    FindDuplicates,
    ReplaceWithClone,
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
    AskAboutCard {
        card: Provenance,
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
    ScanNotice,
    SmallGroups {
        path: PathBuf,
    },
    PreviewTargets {
        path: PathBuf,
    },
    ExportCsv {
        path: PathBuf,
    },
    HiddenSpace,
    TmutilCommands,
    PurgeSpace {
        timeout_seconds: u64,
    },
}
