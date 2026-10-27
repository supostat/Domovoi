use std::path::PathBuf;

use crate::deny_reason::DenyReason;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteRefusal {
    pub path: PathBuf,
    pub reason: DenyReason,
}
