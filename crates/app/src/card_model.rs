use serde::Deserialize;

use crate::held_line::held_line;
use crate::provenance::Provenance;
use crate::safety_badge::SafetyBadge;
use crate::strings::{CLOSE_OWNER, CLOSE_OWNER_SUFFIX};
use crate::tray_target::TrayTarget;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardModel {
    pub title: String,
    pub paths: Vec<String>,
    pub explanation: String,
    pub level: SafetyBadge,
    pub provenance: Provenance,
    pub amount: u64,
    #[serde(default)]
    pub held_by_snapshots: u64,
    #[serde(default)]
    pub held_by_clones: u64,
    #[serde(default)]
    pub held_by_hard_links: u64,
    #[serde(default)]
    pub owner_running: Option<String>,
    #[serde(default)]
    pub blocked: Option<String>,
    #[serde(default)]
    pub vendor_command: Option<String>,
    #[serde(default)]
    pub age: Option<String>,
    #[serde(default)]
    pub selected: bool,
    #[serde(skip)]
    pub targets: Vec<TrayTarget>,
}

impl CardModel {
    pub fn selectable(&self) -> bool {
        self.owner_running.is_none() && self.vendor_command.is_none() && self.amount > 0
    }

    pub fn held_line(&self) -> Option<String> {
        held_line(self.held_by_snapshots, self.held_by_clones, self.held_by_hard_links)
    }

    pub fn owner_line(&self) -> Option<String> {
        self.owner_running
            .as_ref()
            .map(|owner| format!("{CLOSE_OWNER} {owner}, {CLOSE_OWNER_SUFFIX}"))
    }
}
