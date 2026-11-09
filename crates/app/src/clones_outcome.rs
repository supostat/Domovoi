use std::path::PathBuf;

use domovoi_dups::CloneReplaceError;

#[derive(Debug, Default)]
pub struct ClonesOutcome {
    pub freed: u64,
    pub failures: Vec<(PathBuf, CloneReplaceError)>,
    pub stopped: bool,
}
