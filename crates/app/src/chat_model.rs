use domovoi_agent::{AgentAnswer, AgentError, ToolStep};
use domovoi_grammar::Tool;
use domovoi_inference_proto::ErrorCode;
use serde::Deserialize;

use crate::answer_item::AnswerItem;
use crate::answer_target::AnswerTarget;
use crate::availability::Availability;
use crate::chat_entry::ChatEntry;
use crate::strings::{
    ANSWER_CANCELLED, ANSWER_FAILED, ANSWER_LOW_MEMORY, ANSWER_OUTSIDE_SCHEMA, ANSWER_STALLED, ANSWER_TOO_LONG,
    SERVICE_UNAVAILABLE, rule_title,
};
use crate::tilde::tilde;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatModel {
    #[serde(skip)]
    pub availability: Availability,
    pub entries: Vec<ChatEntry>,
    #[serde(default)]
    pub draft: String,
}

impl ChatModel {
    pub fn ask(&mut self, question: &str) {
        self.entries.push(ChatEntry::Question {
            text: question.to_owned(),
        });
    }

    pub fn record_step(&mut self, step: &ToolStep) {
        let arguments: Vec<String> = step
            .arguments
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect();
        self.entries.push(ChatEntry::ToolCall {
            tool: step.tool.name().to_owned(),
            arguments: arguments.join(", "),
            found: (step.tool == Tool::QueryIndex).then_some(step.nodes.len() as u64),
        });
    }

    pub fn record_answer(&mut self, answer: &AgentAnswer) {
        let mut items: Vec<(&str, AnswerItem)> = Vec::new();
        for action in &answer.actions {
            match items.iter_mut().find(|(rule_id, _)| *rule_id == action.rule_id) {
                Some((_, item)) => item.bytes += action.allocated,
                None => items.push((
                    action.rule_id,
                    AnswerItem {
                        title: rule_title(action.rule_id).unwrap_or(action.rule_id).to_owned(),
                        bytes: action.allocated,
                    },
                )),
            }
        }
        self.entries.push(ChatEntry::Answer {
            text: answer.text.clone(),
            items: items.into_iter().map(|(_, item)| item).collect(),
            targets: answer
                .actions
                .iter()
                .map(|action| AnswerTarget {
                    path: action.path.clone(),
                    rule_id: action.rule_id.to_owned(),
                })
                .collect(),
        });
        self.entries
            .extend(answer.hypotheses.iter().map(|hypothesis| ChatEntry::Hypothesis {
                path: tilde(&hypothesis.path),
                text: hypothesis.explanation.clone(),
            }));
    }

    pub fn record_failure(&mut self, error: &AgentError) {
        let text = match error {
            AgentError::Cancelled => ANSWER_CANCELLED,
            AgentError::Backend(failure) if failure.code == ErrorCode::LoadCancelled => ANSWER_CANCELLED,
            AgentError::Backend(failure) if failure.code == ErrorCode::Stalled => ANSWER_STALLED,
            AgentError::Backend(failure) if failure.code == ErrorCode::Unavailable => SERVICE_UNAVAILABLE,
            AgentError::Backend(failure) if failure.code == ErrorCode::OutOfMemory => ANSWER_LOW_MEMORY,
            AgentError::Backend(_) => ANSWER_FAILED,
            AgentError::InvalidResponse { .. } => ANSWER_OUTSIDE_SCHEMA,
            AgentError::ContextOverflow { .. } => ANSWER_TOO_LONG,
        };
        self.entries.push(ChatEntry::Failure { text: text.to_owned() });
    }

    pub fn record_problem(&mut self, text: &str) {
        self.entries.push(ChatEntry::Failure { text: text.to_owned() });
    }
}
