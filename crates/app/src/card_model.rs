use std::fmt::Write;

use serde::Deserialize;

use crate::held_line::held_line;
use crate::provenance::Provenance;
use crate::safety_badge::SafetyBadge;
use crate::strings::{AND_MORE, CLOSE_OWNER, CLOSE_OWNER_SUFFIX};

const REASON_LINES: usize = 3;
use crate::tray_target::TrayTarget;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardModel {
    pub title: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub path_reasons: Vec<Option<String>>,
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

    pub fn reasons_line(&self) -> Option<String> {
        let reasons: Vec<String> = self
            .paths
            .iter()
            .zip(&self.path_reasons)
            .filter_map(|(path, reason)| reason.as_ref().map(|reason| format!("{path} — {reason}")))
            .collect();
        if reasons.is_empty() {
            return None;
        }
        let mut shown = reasons[..reasons.len().min(REASON_LINES)].join("; ");
        if reasons.len() > REASON_LINES {
            write!(shown, "; {AND_MORE} {}", reasons.len() - REASON_LINES).expect("a string takes the count");
        }
        Some(shown)
    }

    pub fn owner_line(&self) -> Option<String> {
        self.owner_running
            .as_ref()
            .map(|owner| format!("{CLOSE_OWNER} {owner}, {CLOSE_OWNER_SUFFIX}"))
    }
}
