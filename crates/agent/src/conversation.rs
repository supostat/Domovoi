use domovoi_grammar::{ChatMessage, ChatRole, ChatTemplate};
use domovoi_inference::InferenceBackend;

use crate::agent_error::AgentError;
use crate::agent_limits::AgentLimits;

const TRIMMED_RESULT: &str = r#"{"trimmed":true}"#;
const MESSAGES_PER_PAST_TURN: usize = 2;

pub(crate) struct Conversation {
    messages: Vec<ChatMessage>,
    past_messages: usize,
}

impl Conversation {
    pub(crate) fn opening(past: Vec<ChatMessage>, content: String) -> Self {
        let past_messages = past.len();
        let mut messages = past;
        messages.push(ChatMessage::user(content));
        Self {
            messages,
            past_messages,
        }
    }

    pub(crate) fn push_tool_exchange(&mut self, call: String, result: String) {
        self.messages.push(ChatMessage::assistant(call));
        self.messages.push(ChatMessage::tool(result));
    }

    pub(crate) fn render(
        &mut self,
        template: ChatTemplate,
        system: &str,
        backend: &mut dyn InferenceBackend,
        limits: &AgentLimits,
    ) -> Result<String, AgentError> {
        loop {
            let prompt = template.render(system, &self.messages);
            let prompt_tokens = backend.count_tokens(&prompt).map_err(AgentError::Backend)?;
            if prompt_tokens + limits.max_tokens_per_step <= limits.context_length {
                return Ok(prompt);
            }
            if !self.trim_oldest_tool_result() && !self.forget_oldest_past_turn() {
                return Err(AgentError::ContextOverflow {
                    prompt_tokens,
                    context_length: limits.context_length,
                });
            }
        }
    }

    fn trim_oldest_tool_result(&mut self) -> bool {
        let Some(oldest) = self
            .messages
            .iter_mut()
            .find(|message| message.role == ChatRole::Tool && message.content != TRIMMED_RESULT)
        else {
            return false;
        };
        TRIMMED_RESULT.clone_into(&mut oldest.content);
        true
    }

    fn forget_oldest_past_turn(&mut self) -> bool {
        if self.past_messages < MESSAGES_PER_PAST_TURN {
            return false;
        }
        self.messages.drain(..MESSAGES_PER_PAST_TURN);
        self.past_messages -= MESSAGES_PER_PAST_TURN;
        true
    }
}
