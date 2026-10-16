use crate::final_answer::FinalAnswer;
use crate::tool_call::ToolCall;

#[derive(Clone, Debug, PartialEq)]
pub enum ModelResponse {
    ToolCall(ToolCall),
    Final(FinalAnswer),
}
