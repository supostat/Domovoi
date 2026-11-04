use std::path::PathBuf;

use crate::tray_summary::TraySummary;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CachedTraySummary {
    pub generation: u64,
    pub items: Vec<PathBuf>,
    pub summary: TraySummary,
}
