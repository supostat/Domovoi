use std::path::{Path, PathBuf};

use domovoi_index::{Index, NodeId};
use domovoi_rules::{Denylist, Tray};
use domovoi_theme::{format_count, format_size};

use crate::deny_reason_text::deny_reason_text;
use crate::drop_hover::DropHover;
use crate::drop_kind::DropKind;
use crate::drop_refusal::DropRefusal;
use crate::strings::{DROP_ADD, DROP_ALREADY, DROP_DENIED, ESTIMATE_MARK, NOT_ADDED, OBJECTS, OF};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DropVerdict {
    pub allowed: Vec<NodeId>,
    pub refused: Vec<DropRefusal>,
    pub already: usize,
    pub allowed_bytes: u64,
}

impl DropVerdict {
    pub fn judge(index: &Index, denylist: &Denylist, tray: &Tray, paths: &[PathBuf]) -> Self {
        let mut verdict = Self::default();
        for path in paths {
            let Some(node) = index.find(path) else {
                continue;
            };
            if let Some(reason) = denylist.verdict(index, node) {
                verdict.refused.push(DropRefusal {
                    name: display_name(&index.path(node)),
                    reason,
                });
            } else if tray.covers(&index.path(node)) {
                verdict.already += 1;
            } else {
                verdict.allowed_bytes += index.reclaimable_estimate(node).bytes;
                verdict.allowed.push(node);
            }
        }
        verdict
    }

    pub fn hover(&self) -> DropHover {
        if !self.allowed.is_empty() {
            return DropHover {
                kind: DropKind::Add,
                title: format!(
                    "{DROP_ADD} {} · {ESTIMATE_MARK} {}",
                    self.added_count(),
                    format_size(self.allowed_bytes)
                ),
            };
        }
        if self.refused.is_empty() {
            return DropHover {
                kind: DropKind::Already,
                title: DROP_ALREADY.to_owned(),
            };
        }
        let mut reasons: Vec<&str> = Vec::new();
        for refusal in &self.refused {
            let reason = deny_reason_text(&refusal.reason);
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
        DropHover {
            kind: DropKind::Denied,
            title: format!("{DROP_DENIED} {}", reasons.join(", ")),
        }
    }

    pub fn refusal_line(&self) -> Option<String> {
        if self.refused.is_empty() {
            return None;
        }
        let refusals: Vec<String> = self
            .refused
            .iter()
            .map(|refusal| format!("{} — {}", refusal.name, deny_reason_text(&refusal.reason)))
            .collect();
        Some(format!("{NOT_ADDED} {}", refusals.join(" · ")))
    }

    fn added_count(&self) -> String {
        let judged = self.allowed.len() + self.refused.len() + self.already;
        if judged > self.allowed.len() {
            format!("{} {OF} {judged}", self.allowed.len())
        } else {
            format_count(self.allowed.len() as u64, &OBJECTS)
        }
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
