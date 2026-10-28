use std::path::Path;

use domovoi_index::{Index, Reclaimable};
use domovoi_rules::Tray;
use domovoi_theme::{format_count, group_thousands};
use serde::Deserialize;

use crate::delete_outcome::DeleteOutcome;
use crate::delete_status::DeleteStatus;
use crate::held_line::held_line;
use crate::measure_progress::MeasureProgress;
use crate::strings::{DELETE_UNITS, DELETING, FILES, OF, REFINING_SIZE, rule_title};
use crate::tilde::tilde;
use crate::tray_row::TrayRow;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrayModel {
    pub rows: Vec<TrayRow>,
    #[serde(default)]
    pub held_by_snapshots: u64,
    #[serde(default)]
    pub held_by_clones: u64,
    #[serde(default)]
    pub held_by_hard_links: u64,
    #[serde(default)]
    pub measuring: Option<MeasureProgress>,
    #[serde(default)]
    pub deleting: Option<DeleteStatus>,
    #[serde(default)]
    pub deleted: Option<DeleteOutcome>,
}

impl TrayModel {
    pub fn estimated(index: &Index, tray: &Tray) -> Self {
        Self::measured(index, tray, &Self::estimated_sizes(index, tray)).with_progress(MeasureProgress::default())
    }

    pub fn estimated_sizes(index: &Index, tray: &Tray) -> Vec<Reclaimable> {
        tray.items()
            .iter()
            .map(|item| index.reclaimable_estimate(item.node))
            .collect()
    }

    pub fn measured(index: &Index, tray: &Tray, sizes: &[Reclaimable]) -> Self {
        let mut rows: Vec<(Option<String>, TrayRow)> = Vec::new();
        let mut held = Reclaimable::default();
        for (item, reclaimable) in tray.items().iter().zip(sizes.iter().copied()) {
            held += reclaimable;
            let size = reclaimable.bytes;
            let category = index.node(item.node).category;
            match rows
                .iter_mut()
                .find(|(rule, _)| item.rule_id.is_some() && *rule == item.rule_id)
            {
                Some((_, row)) => row.size += size,
                None => rows.push((
                    item.rule_id.clone(),
                    TrayRow {
                        category,
                        title: item
                            .rule_id
                            .as_deref()
                            .and_then(rule_title)
                            .map_or_else(|| file_name(&item.path), str::to_owned),
                        size,
                    },
                )),
            }
        }
        Self {
            rows: rows.into_iter().map(|(_, row)| row).collect(),
            held_by_snapshots: held.held_by_snapshots,
            held_by_clones: held.held_by_clones,
            held_by_hard_links: held.held_by_hard_links,
            measuring: None,
            deleting: None,
            deleted: None,
        }
    }

    #[must_use]
    pub fn with_progress(mut self, progress: MeasureProgress) -> Self {
        self.measuring = Some(progress);
        self
    }

    pub fn measuring_line(&self) -> Option<String> {
        self.measuring
            .map(|progress| format!("{REFINING_SIZE} {}", format_count(progress.files, &FILES)))
    }

    #[must_use]
    pub fn with_deleting(mut self, status: DeleteStatus) -> Self {
        self.measuring = None;
        self.deleting = Some(status);
        self
    }

    pub fn deleting_line(&self) -> Option<String> {
        self.deleting.map(|status| {
            format!(
                "{DELETING} {} {OF} {}",
                group_thousands(status.files_done as u64),
                format_count(status.files_total as u64, &DELETE_UNITS)
            )
        })
    }

    pub fn deleting_share(&self) -> Option<f64> {
        self.deleting.map(|status| {
            if status.files_total == 0 {
                0.0
            } else {
                status.files_done as f64 / status.files_total as f64
            }
        })
    }

    #[must_use]
    pub fn with_deleted(mut self, outcome: DeleteOutcome) -> Self {
        self.measuring = None;
        self.deleting = None;
        self.deleted = Some(outcome);
        self
    }

    pub fn total(&self) -> u64 {
        self.rows.iter().map(|row| row.size).sum()
    }

    pub fn held_line(&self) -> Option<String> {
        held_line(self.held_by_snapshots, self.held_by_clones, self.held_by_hard_links)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| tilde(path), |name| name.to_string_lossy().into_owned())
}
