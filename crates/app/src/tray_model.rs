use std::io;
use std::path::Path;

use domovoi_index::Index;
use domovoi_rules::{TrashFailure, TrashJournal, TrashOutcome, Tray};
use domovoi_theme::format_size;
use serde::Deserialize;

use crate::deny_reason_text::deny_reason_text;
use crate::journal_row::JournalRow;
use crate::strings::{
    MOVED_TO_TRASH, TRASH_MISSING, TRASH_NO_PERMISSION, TRASH_NO_TRASH, TRASH_OTHER, TRASH_OUT_OF_SPACE,
    TRASH_READ_ONLY, rule_title,
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
    pub outcome: Option<TrayOutcome>,
    #[serde(default)]
    pub journal: Vec<JournalRow>,
}

impl TrayModel {
    pub fn build(index: &Index, tray: &Tray, journal: &TrashJournal) -> io::Result<Self> {
        let mut rows: Vec<(Option<String>, TrayRow)> = Vec::new();
        for item in tray.items() {
            let size = index.reclaimable(item.node)?.bytes;
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
        Ok(Self {
            rows: rows.into_iter().map(|(_, row)| row).collect(),
            outcome: None,
            journal: journal_rows(journal),
        })
    }

    pub fn total(&self) -> u64 {
        self.rows.iter().map(|row| row.size).sum()
    }

    #[must_use]
    pub fn with_outcome(mut self, outcomes: &[TrashOutcome], moved: u64, journal: &TrashJournal) -> Self {
        let failures = outcomes
            .iter()
            .filter_map(|outcome| match &outcome.result {
                Ok(_) => None,
                Err(failure) => Some(format!("{} — {}", tilde(&outcome.path), failure_text(failure))),
            })
            .collect();
        self.outcome = Some(TrayOutcome { moved, failures });
        self.journal = journal_rows(journal);
        self
    }

    pub fn moved_line(moved: u64) -> String {
        format!("{MOVED_TO_TRASH} {}", format_size(moved))
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
