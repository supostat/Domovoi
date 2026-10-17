use std::path::PathBuf;

use domovoi_index::{NodeId, SafetyLevel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestedAction {
    pub node: NodeId,
    pub path: PathBuf,
    pub rule_id: &'static str,
    pub level: SafetyLevel,
    pub allocated: u64,
}
