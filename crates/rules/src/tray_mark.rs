use std::path::PathBuf;

use crate::app_removal::RemovalReason;

#[derive(Clone, Debug, Default)]
pub(crate) struct TrayMark {
    pub(crate) rule_id: Option<String>,
    pub(crate) reason: Option<RemovalReason>,
    pub(crate) application: Option<String>,
    pub(crate) project: Option<PathBuf>,
    pub(crate) duplicate_of: Option<PathBuf>,
}
