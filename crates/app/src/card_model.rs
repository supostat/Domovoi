use domovoi_theme::format_size;
use serde::Deserialize;

use crate::provenance::Provenance;
use crate::safety_badge::SafetyBadge;
use crate::strings::{
    CLOSE_OWNER, CLOSE_OWNER_SUFFIX, HELD_BY_CLONES, HELD_BY_HARD_LINKS, HELD_BY_SNAPSHOTS, HELD_PREFIX,
};
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
        let parts: Vec<String> = [
            (self.held_by_snapshots, HELD_BY_SNAPSHOTS),
            (self.held_by_clones, HELD_BY_CLONES),
            (self.held_by_hard_links, HELD_BY_HARD_LINKS),
        ]
        .into_iter()
        .filter(|(bytes, _)| *bytes > 0)
        .map(|(bytes, holder)| format!("{} {holder}", format_size(bytes)))
        .collect();
        (!parts.is_empty()).then(|| format!("{HELD_PREFIX} {}", parts.join(" · ")))
    }

    pub fn owner_line(&self) -> Option<String> {
        self.owner_running
            .as_ref()
            .map(|owner| format!("{CLOSE_OWNER} {owner}, {CLOSE_OWNER_SUFFIX}"))
    }
}
