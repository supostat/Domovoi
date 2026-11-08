use std::path::PathBuf;

use domovoi_index::NodeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayTarget {
    pub node: NodeId,
    pub rule_id: Option<String>,
    pub project: Option<PathBuf>,
}
