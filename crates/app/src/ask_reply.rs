use domovoi_agent::{AgentAnswer, AgentError};
#[cfg(feature = "claude-code")]
use domovoi_agent::{ClaudeAnswer, ClaudeCodeError};

use crate::chat_model::ChatModel;

#[derive(Debug)]
pub enum AskReply {
    Local(Result<AgentAnswer, AgentError>),
    #[cfg(feature = "claude-code")]
    Claude(Result<ClaudeAnswer, ClaudeCodeError>),
}

impl AskReply {
    pub fn record(&self, chat: &mut ChatModel) {
        match self {
            Self::Local(Ok(answer)) => chat.record_answer(answer),
            Self::Local(Err(error)) => chat.record_failure(error),
            #[cfg(feature = "claude-code")]
            Self::Claude(Ok(answer)) => {
                chat.record_answer(&answer.answer);
                chat.claude_session = Some(answer.session_id.clone());
            }
            #[cfg(feature = "claude-code")]
            Self::Claude(Err(error)) => chat.record_claude_failure(error),
        }
    }
}
