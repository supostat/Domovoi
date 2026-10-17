use domovoi_index::NodeId;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ToolOutcome {
    pub(crate) result: Value,
    pub(crate) nodes: Vec<NodeId>,
}
