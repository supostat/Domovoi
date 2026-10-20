use std::path::Path;

use domovoi_index::{Index, Reclaimable};
use domovoi_rules::{TrashFailure, TrashJournal, TrashOutcome, Tray};
use domovoi_theme::{format_count, group_thousands};
use serde::Deserialize;

use crate::deny_reason_text::deny_reason_text;
use crate::held_line::held_line;
use crate::journal_row::JournalRow;
use crate::measure_progress::MeasureProgress;
use crate::purge_outcome::PurgeOutcome;
use crate::purge_status::PurgeStatus;
use crate::strings::{
    FILES, OF, PURGE_ITEMS, PURGED, PURGED_OBJECTS, PURGING, REFINING_SIZE, TRASH_MISSING, TRASH_NO_PERMISSION,
    TRASH_NO_TRASH, TRASH_OTHER, TRASH_OUT_OF_SPACE, TRASH_READ_ONLY, rule_title,
};
use crate::tilde::tilde;
use crate::tray_outcome::TrayOutcome;
use crate::tray_row::TrayRow;

pub(crate) const RECENT_LIMIT: usize = 5;

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
    pub outcome: Option<TrayOutcome>,
    #[serde(default)]
    pub journal: Vec<JournalRow>,
    #[serde(default)]
    pub purging: Option<PurgeStatus>,
    #[serde(default)]
    pub purged: Option<PurgeOutcome>,
}

impl TrayModel {
    pub fn estimated(index: &Index, tray: &Tray, journal: &TrashJournal) -> Self {
        Self::measured(index, tray, journal, &Self::estimated_sizes(index, tray))
            .with_progress(MeasureProgress::default())
    }

    pub fn estimated_sizes(index: &Index, tray: &Tray) -> Vec<Reclaimable> {
        tray.items()
            .iter()
            .map(|item| index.reclaimable_estimate(item.node))
            .collect()
    }

    pub fn measured(index: &Index, tray: &Tray, journal: &TrashJournal, sizes: &[Reclaimable]) -> Self {
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
            outcome: None,
            journal: journal_rows(journal),
            purging: None,
            purged: None,
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
    pub fn with_purge(mut self, status: PurgeStatus) -> Self {
        self.purging = Some(status);
        self
    }

    #[must_use]
    pub fn with_purged(mut self, outcome: PurgeOutcome, journal: &TrashJournal) -> Self {
        self.purging = None;
        self.purged = Some(outcome);
        self.journal = journal_rows(journal);
        self
    }

    pub fn purging_line(&self) -> Option<String> {
        self.purging.map(|status| {
            format!(
                "{PURGING} {} {OF} {}",
                group_thousands(status.done as u64),
                format_count(status.total as u64, &PURGE_ITEMS)
            )
        })
    }

    pub fn purged_line(&self) -> Option<String> {
        self.purged
            .as_ref()
            .map(|outcome| format!("{PURGED} {}", format_count(outcome.removed as u64, &PURGED_OBJECTS)))
    }

    pub fn total(&self) -> u64 {
        self.rows.iter().map(|row| row.size).sum()
    }

    pub fn held_line(&self) -> Option<String> {
        held_line(self.held_by_snapshots, self.held_by_clones, self.held_by_hard_links)
    }

    #[must_use]
    pub fn with_outcome(mut self, outcomes: &[TrashOutcome], journal: &TrashJournal) -> Self {
        let failures = outcomes
            .iter()
            .filter_map(|outcome| match &outcome.result {
                Ok(_) => None,
                Err(failure) => Some(format!("{} — {}", tilde(&outcome.path), failure_text(failure))),
            })
            .collect();
        self.outcome = Some(TrayOutcome {
            moved: self.total(),
            estimated: self.measuring.is_some(),
            failures,
        });
        self.measuring = None;
        self.journal = journal_rows(journal);
        self
    }
}

fn journal_rows(journal: &TrashJournal) -> Vec<JournalRow> {
    journal
        .entries()
        .iter()
        .enumerate()
        .rev()
        .take(RECENT_LIMIT)
        .map(|(position, entry)| JournalRow {
            title: entry
                .rule_id
                .as_deref()
                .and_then(rule_title)
                .map_or_else(|| file_name(Path::new(&entry.original)), str::to_owned),
            path: tilde(Path::new(&entry.original)),
            size: entry.alloc_size,
            position,
        })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| tilde(path), |name| name.to_string_lossy().into_owned())
}

fn failure_text(failure: &TrashFailure) -> &'static str {
    match failure {
        TrashFailure::Denied(reason) => deny_reason_text(reason),
        TrashFailure::NoPermission => TRASH_NO_PERMISSION,
        TrashFailure::ReadOnlyVolume => TRASH_READ_ONLY,
        TrashFailure::Missing => TRASH_MISSING,
        TrashFailure::NoTrashOnVolume => TRASH_NO_TRASH,
        TrashFailure::OutOfSpace => TRASH_OUT_OF_SPACE,
        TrashFailure::Other { .. } => TRASH_OTHER,
    }
}
