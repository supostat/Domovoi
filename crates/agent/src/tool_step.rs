use domovoi_grammar::Tool;
use domovoi_index::NodeId;
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct ToolStep {
    pub tool: Tool,
    pub arguments: Map<String, Value>,
    pub result: Value,
    pub nodes: Vec<NodeId>,
}
