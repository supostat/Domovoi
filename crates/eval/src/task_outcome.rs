use domovoi_grammar::Tool;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TaskOutcome {
    pub id: String,
    pub tools: Vec<String>,
    pub first_tool: Option<String>,
    pub schema_valid: bool,
    pub answer: Option<String>,
    pub error: Option<String>,
    pub score: Option<u8>,
    pub judge_reason: Option<String>,
}

impl TaskOutcome {
    pub fn first_tool_is(&self, tool: Tool) -> bool {
        self.first_tool.as_deref() == Some(tool.name())
    }
}
