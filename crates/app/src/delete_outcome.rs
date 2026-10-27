use domovoi_rules::{DeleteFailure, DeleteRefusal, DeleteReport};
use domovoi_theme::{format_count, format_size};
use serde::Deserialize;

use crate::deny_reason_text::deny_reason_text;
use crate::strings::{
    DELETE_MISSING, DELETE_NO_PERMISSION, DELETE_OTHER, DELETE_READ_ONLY, DELETE_REPLACED, DELETED, FREED, NOT_STARTED,
    OBJECTS, STOPPED_MIDWAY,
};
use crate::tilde::tilde;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteOutcome {
    pub objects: usize,
    pub freed: u64,
    #[serde(default)]
    pub failures: Vec<String>,
}

impl DeleteOutcome {
    pub fn from_report(report: &DeleteReport, refusals: &[DeleteRefusal]) -> Self {
        let mut failures: Vec<String> = refusals
            .iter()
            .map(|refusal| format!("{} — {}", tilde(&refusal.path), deny_reason_text(&refusal.reason)))
            .collect();
        failures.extend(
            report
                .failed
                .iter()
                .map(|(subject, failure)| format!("{} — {}", tilde(&subject.path), failure_text(failure))),
        );
        failures.extend(
            report
                .interrupted
                .iter()
                .map(|subject| format!("{} — {STOPPED_MIDWAY}", tilde(&subject.path))),
        );
        failures.extend(
            report
                .untouched
                .iter()
                .map(|subject| format!("{} — {NOT_STARTED}", tilde(&subject.path))),
        );
        Self {
            objects: report.deleted.len(),
            freed: report.freed_bytes,
            failures,
        }
    }

    pub fn line(&self) -> String {
        format!(
            "{DELETED} {}, {FREED} {}",
            format_count(self.objects as u64, &OBJECTS),
            format_size(self.freed)
        )
    }
}

fn failure_text(failure: &DeleteFailure) -> &'static str {
    match failure {
        DeleteFailure::Missing => DELETE_MISSING,
        DeleteFailure::Replaced => DELETE_REPLACED,
        DeleteFailure::NoPermission => DELETE_NO_PERMISSION,
        DeleteFailure::ReadOnlyVolume => DELETE_READ_ONLY,
        DeleteFailure::Other { .. } => DELETE_OTHER,
    }
}
