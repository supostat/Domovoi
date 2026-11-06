use std::fs;
use std::path::Path;

use domovoi_index::TimeMachineSettings;
use domovoi_render::{DiagramTree, MarkedNodes, ROOT};
use serde::Deserialize;

use crate::access_state::AccessState;
use crate::ai_fixture::AiFixture;
use crate::availability::Availability;
use crate::care_models::CareModels;
use crate::delete_status::DeleteStatus;
use crate::measure_progress::MeasureProgress;
use crate::outline_fixture::OutlineFixture;
use crate::scan_status::ScanStatus;
use crate::screen::Screen;
use crate::shell_data::ShellData;
use crate::tray_variant::TrayVariant;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellFixture {
    pub disk_name: String,
    pub access: AccessState,
    pub disk_total_bytes: u64,
    #[serde(default)]
    pub in_cloud_bytes: u64,
    #[serde(default)]
    pub hidden_folders: u64,
    #[serde(default)]
    pub snapshots_present: bool,
    #[serde(default)]
    pub other_user_homes: u64,
    #[serde(default)]
    pub time_machine_backups: bool,
    pub scan: ScanStatus,
    pub tree: OutlineFixture,
    pub care: CareModels,
    pub tray_measuring: MeasureProgress,
    pub tray_deleting: DeleteStatus,
    pub ai: AiFixture,
}

impl ShellFixture {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn shell_data(&self, screen: Screen) -> Result<ShellData, String> {
        let tree = DiagramTree::from_outline(&self.tree.outline()?);
        let used = tree.node(ROOT).size;
        let mut care = self.care.clone();
        if screen == Screen::Tray(TrayVariant::Measuring) {
            care.tray = care.tray.with_progress(self.tray_measuring);
        }
        if screen == Screen::Tray(TrayVariant::Deleting) {
            care.tray = care.tray.with_deleting(self.tray_deleting);
        }
        Ok(ShellData {
            disk_name: self.disk_name.clone(),
            access: self.access,
            free_bytes: self.disk_total_bytes.saturating_sub(used),
            tree,
            disk_total_bytes: self.disk_total_bytes,
            in_cloud_bytes: self.in_cloud_bytes,
            scan: self.scan.clone(),
            care,
            ai: self.ai.models_for(screen),
            care_ready: true,
            rescan_allowed: !matches!(
                screen,
                Screen::Scanning | Screen::Assistant(Availability::Answering) | Screen::Tray(_)
            ),
            hidden_folders: self.hidden_folders,
            snapshots_present: self.snapshots_present,
            other_user_homes: self.other_user_homes,
            time_machine: TimeMachineSettings {
                auto_backup: Some(self.time_machine_backups),
                mobile_backups: None,
            },
            marked: MarkedNodes::default(),
        })
    }
}
