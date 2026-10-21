use crate::agent_answer::AgentAnswer;

#[derive(Clone, Debug, PartialEq)]
pub struct ClaudeAnswer {
    pub answer: AgentAnswer,
    pub session_id: String,
}
