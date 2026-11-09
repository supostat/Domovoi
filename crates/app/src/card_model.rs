use std::fmt::Write;

use domovoi_theme::format_count;
use serde::Deserialize;

use crate::held_line::held_line;
use crate::project_row::ProjectRow;
use crate::provenance::Provenance;
use crate::safety_badge::SafetyBadge;
use crate::strings::{AND_MORE, CLOSE_OWNER, CLOSE_OWNER_SUFFIX, PROJECTS};

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
    #[serde(default)]
    pub rows: Vec<ProjectRow>,
    #[serde(skip)]
    pub targets: Vec<TrayTarget>,
}

impl CardModel {
    pub fn is_same_card_as(&self, other: &Self) -> bool {
        match (&self.provenance, &other.provenance) {
            (Provenance::Hypothesis, Provenance::Hypothesis) => self.paths == other.paths,
            (provenance, other_provenance) => provenance == other_provenance,
        }
    }

    pub fn selectable(&self) -> bool {
        self.owner_running.is_none() && self.vendor_command.is_none() && self.amount > 0
    }

    pub fn selected_amount(&self) -> u64 {
        if self.rows.is_empty() {
            if self.selected && self.selectable() {
                self.amount
            } else {
                0
            }
        } else {
            self.rows.iter().filter(|row| row.selected).map(|row| row.amount).sum()
        }
    }

    pub fn selected_targets(&self) -> Vec<TrayTarget> {
        if self.rows.is_empty() {
            if self.selected && self.selectable() {
                self.targets.clone()
            } else {
                Vec::new()
            }
        } else {
            self.rows
                .iter()
                .filter(|row| row.selected)
                .flat_map(|row| row.targets.iter().cloned())
                .collect()
        }
    }

    pub fn path_line(&self) -> String {
        if self.rows.is_empty() {
            self.paths.join(", ")
        } else {
            format_count(self.rows.len() as u64, &PROJECTS)
        }
    }

    pub fn every_row_selected(&self) -> bool {
        every_selectable_selected(self.rows.iter())
    }

    pub fn every_group_row_selected(&self, stale: bool) -> bool {
        every_selectable_selected(self.rows.iter().filter(|row| row.stale == stale))
    }

    pub fn some_row_selected(&self) -> bool {
        self.rows.iter().any(|row| row.selected)
    }

    pub fn toggle_rows(&mut self) {
        let selected = !self.every_row_selected();
        for row in self.rows.iter_mut().filter(|row| row.selectable()) {
            row.selected = selected;
        }
        self.selected = self.every_row_selected();
    }

    pub fn toggle_row(&mut self, position: usize) {
        if let Some(row) = self.rows.get_mut(position).filter(|row| row.selectable()) {
            row.selected = !row.selected;
        }
        self.selected = self.every_row_selected();
    }

    pub fn toggle_group(&mut self, stale: bool) {
        let selected = !self.every_group_row_selected(stale);
        for row in self
            .rows
            .iter_mut()
            .filter(|row| row.stale == stale && row.selectable())
        {
            row.selected = selected;
        }
        self.selected = self.every_row_selected();
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

fn every_selectable_selected<'rows>(rows: impl Iterator<Item = &'rows ProjectRow>) -> bool {
    let mut selectable = rows.filter(|row| row.selectable()).peekable();
    selectable.peek().is_some() && selectable.all(|row| row.selected)
}
