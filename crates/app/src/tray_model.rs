use std::path::{Path, PathBuf};

use domovoi_index::{Index, NodeId};
use domovoi_rules::{Tray, TrayItem};
use domovoi_theme::{format_count, group_thousands};
use serde::Deserialize;

use crate::delete_outcome::DeleteOutcome;
use crate::delete_status::DeleteStatus;
use crate::held_line::held_line;
use crate::measure_progress::MeasureProgress;
use crate::owner_reason::removal_phrase;
use crate::strings::{AND_MORE, DELETE_UNITS, DELETING, FILES, OF, REFINING_SIZE, WITH_ITS_FILES, rule_title};
use crate::tilde::tilde;
use crate::tray_row::TrayRow;
use crate::tray_sizes::TraySizes;

const APP_FILE_LINES: usize = 6;

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
    pub freed_together: u64,
    #[serde(default)]
    pub measuring: Option<MeasureProgress>,
    #[serde(default)]
    pub deleting: Option<DeleteStatus>,
    #[serde(default)]
    pub deleted: Option<DeleteOutcome>,
    #[serde(skip)]
    pub row_nodes: Vec<Vec<NodeId>>,
}

impl TrayModel {
    pub fn estimated(index: &Index, tray: &Tray) -> Self {
        Self::measured(index, tray, &Self::estimated_sizes(index, tray)).with_progress(MeasureProgress::default())
    }

    pub fn estimated_sizes(index: &Index, tray: &Tray) -> TraySizes {
        let nodes: Vec<NodeId> = tray.items().iter().map(|item| item.node).collect();
        TraySizes {
            items: nodes.iter().map(|node| index.reclaimable_estimate(*node)).collect(),
            together: index.reclaimable_estimate_of(&nodes),
        }
    }

    pub fn measured(index: &Index, tray: &Tray, sizes: &TraySizes) -> Self {
        let mut rows: Vec<(Option<RowGroup>, TrayRow, Vec<NodeId>)> = Vec::new();
        for (item, reclaimable) in tray.items().iter().zip(&sizes.items) {
            let size = reclaimable.bytes;
            let group = RowGroup::of(item);
            let detail = match (item.reason, &item.project) {
                (Some(reason), _) => Some(format!("{} · {}", tilde(&item.path), removal_phrase(reason))),
                (None, Some(_)) => Some(tilde(&item.path)),
                (None, None) => None,
            };
            if let Some((_, row, nodes)) = rows
                .iter_mut()
                .find(|(existing, _, _)| group.is_some() && *existing == group)
            {
                row.size += size;
                row.details.extend(detail);
                nodes.push(item.node);
            } else {
                let row = TrayRow {
                    category: index.node(item.node).category,
                    title: row_title(tray, item),
                    size,
                    details: detail.into_iter().collect(),
                    unmarked: unmarked_paths(tray, item),
                };
                rows.push((group, row, vec![item.node]));
            }
        }
        let (rows, row_nodes): (Vec<TrayRow>, Vec<Vec<NodeId>>) = rows
            .into_iter()
            .map(|(_, mut row, nodes)| {
                shorten_details(&mut row.details);
                (row, nodes)
            })
            .unzip();
        Self {
            rows,
            held_by_snapshots: sizes.together.held_by_snapshots,
            held_by_clones: sizes.together.held_by_clones,
            held_by_hard_links: sizes.together.held_by_hard_links,
            freed_together: sizes.freed_together(),
            measuring: None,
            deleting: None,
            deleted: None,
            row_nodes,
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
        self.rows.iter().map(|row| row.size).sum::<u64>() + self.freed_together
    }

    pub fn held_line(&self) -> Option<String> {
        held_line(self.held_by_snapshots, self.held_by_clones, self.held_by_hard_links)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RowGroup {
    Rule(String),
    Application(String),
    Project(PathBuf),
}

impl RowGroup {
    fn of(item: &TrayItem) -> Option<Self> {
        match (&item.application, &item.project, &item.rule_id) {
            (Some(application), _, _) => Some(Self::Application(application.clone())),
            (None, Some(project), _) => Some(Self::Project(project.clone())),
            (None, None, Some(rule)) => Some(Self::Rule(rule.clone())),
            (None, None, None) => None,
        }
    }
}

fn row_title(tray: &Tray, item: &TrayItem) -> String {
    if let Some(project) = &item.project {
        return file_name(project);
    }
    if let Some(application) = &item.application {
        let name = tray
            .applications()
            .iter()
            .find(|known| &known.bundle_id == application)
            .map_or(application.as_str(), |known| known.name.as_str());
        return format!("{name} {WITH_ITS_FILES}");
    }
    item.rule_id
        .as_deref()
        .and_then(rule_title)
        .map_or_else(|| file_name(&item.path), str::to_owned)
}

fn unmarked_paths(tray: &Tray, item: &TrayItem) -> Vec<String> {
    let Some(application) = &item.application else {
        return Vec::new();
    };
    tray.applications()
        .iter()
        .find(|known| &known.bundle_id == application)
        .map_or_else(Vec::new, |known| {
            known.name_only.iter().map(|path| tilde(path)).collect()
        })
}

fn shorten_details(details: &mut Vec<String>) {
    if details.len() > APP_FILE_LINES {
        let hidden = details.len() - APP_FILE_LINES;
        details.truncate(APP_FILE_LINES);
        details.push(format!("{AND_MORE} {hidden}"));
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| tilde(path), |name| name.to_string_lossy().into_owned())
}
