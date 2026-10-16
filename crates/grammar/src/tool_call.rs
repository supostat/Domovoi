use serde_json::{Map, Value};

use crate::tool::Tool;

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub tool: Tool,
    pub arguments: Map<String, Value>,
}
