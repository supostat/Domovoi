use serde::Deserialize;

use crate::chat_message::ChatMessage;
use crate::chat_role::ChatRole;
use crate::prompt_family::PromptFamily;

const TURN_START: &str = "<|im_start|>";
const TURN_END: &str = "<|im_end|>\n";
const EMPTY_THINKING: &str = "<think>\n\n</think>\n\n";
const TOOL_RESPONSE_START: &str = "\n<tool_response>\n";
const TOOL_RESPONSE_END: &str = "\n</tool_response>";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatTemplate {
    pub family: PromptFamily,
    pub empty_think_before_answer: bool,
    pub empty_think_in_tool_steps: bool,
}

impl ChatTemplate {
    pub fn render(self, system: &str, messages: &[ChatMessage]) -> String {
        match self.family {
            PromptFamily::ChatMl => self.render_chat_ml(system, messages),
        }
    }

    fn render_chat_ml(self, system: &str, messages: &[ChatMessage]) -> String {
        let last_query = messages
            .iter()
            .rposition(|message| message.role == ChatRole::User)
            .expect("a conversation holds a user query");
        assert!(
            messages
                .last()
                .is_some_and(|message| message.role != ChatRole::Assistant),
            "a prompt ends with a user query or a tool result"
        );
        let mut prompt = String::new();
        open_turn(&mut prompt, "system\n");
        prompt.push_str(system.trim());
        prompt.push_str(TURN_END);
        for (position, message) in messages.iter().enumerate() {
            let content = message.content.trim();
            match message.role {
                ChatRole::User => {
                    open_turn(&mut prompt, "user\n");
                    prompt.push_str(content);
                    prompt.push_str(TURN_END);
                }
                ChatRole::Assistant => {
                    open_turn(&mut prompt, "assistant\n");
                    if self.empty_think_in_tool_steps && position > last_query {
                        prompt.push_str(EMPTY_THINKING);
                    }
                    prompt.push_str(content);
                    prompt.push_str(TURN_END);
                }
                ChatRole::Tool => {
                    let continues_tool_turn = position > 0 && messages[position - 1].role == ChatRole::Tool;
                    if !continues_tool_turn {
                        open_turn(&mut prompt, "user");
                    }
                    prompt.push_str(TOOL_RESPONSE_START);
                    prompt.push_str(content);
                    prompt.push_str(TOOL_RESPONSE_END);
                    let tool_turn_continues = messages
                        .get(position + 1)
                        .is_some_and(|next| next.role == ChatRole::Tool);
                    if !tool_turn_continues {
                        prompt.push_str(TURN_END);
                    }
                }
            }
        }
        open_turn(&mut prompt, "assistant\n");
        if self.empty_think_before_answer {
            prompt.push_str(EMPTY_THINKING);
        }
        prompt
    }
}

fn open_turn(prompt: &mut String, header: &str) {
    prompt.push_str(TURN_START);
    prompt.push_str(header);
}
