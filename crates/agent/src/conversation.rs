use domovoi_grammar::{ChatMessage, ChatRole, ChatTemplate};
use domovoi_inference::InferenceBackend;

use crate::agent_error::AgentError;
use crate::agent_limits::AgentLimits;

const TRIMMED_RESULT: &str = r#"{"trimmed":true}"#;

pub(crate) struct Conversation {
    messages: Vec<ChatMessage>,
}

impl Conversation {
    pub(crate) fn opening(content: String) -> Self {
        Self {
            messages: vec![ChatMessage::user(content)],
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
            let Some(oldest) = self
                .messages
                .iter_mut()
                .find(|message| message.role == ChatRole::Tool && message.content != TRIMMED_RESULT)
            else {
                return Err(AgentError::ContextOverflow {
                    prompt_tokens,
                    context_length: limits.context_length,
                });
            };
            TRIMMED_RESULT.clone_into(&mut oldest.content);
        }
    }
}
