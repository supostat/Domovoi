use std::path::PathBuf;

use crate::orphan_reason::OrphanReason;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Orphan {
    pub path: PathBuf,
    pub reason: OrphanReason,
}
