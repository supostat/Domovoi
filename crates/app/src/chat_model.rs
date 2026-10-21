use domovoi_agent::{AgentAnswer, AgentError, PastTurn, ToolStep};
use domovoi_grammar::Tool;
use domovoi_inference_proto::ErrorCode;
use serde::Deserialize;
use serde_json::Value;

use crate::answer_item::AnswerItem;
use crate::answer_target::AnswerTarget;
use crate::availability::Availability;
use crate::chat_entry::ChatEntry;
use crate::strings::{
    ANSWER_CANCELLED, ANSWER_FAILED, ANSWER_LOW_MEMORY, ANSWER_OUTSIDE_SCHEMA, ANSWER_STALLED, ANSWER_TOO_LONG,
    SERVICE_UNAVAILABLE, rule_title,
};
use crate::tilde::tilde;

const PAST_TURNS: usize = 6;
const PATH_ARGUMENTS: [&str; 2] = ["path", "path_prefix"];

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
            paths: PATH_ARGUMENTS
                .iter()
                .filter_map(|name| step.arguments.get(*name).and_then(Value::as_str))
                .map(str::to_owned)
                .collect(),
        });
    }

    pub fn past_turns(&self) -> Vec<PastTurn> {
        let mut turns: Vec<PastTurn> = Vec::new();
        let mut open: Option<PastTurn> = None;
        for entry in &self.entries {
            match entry {
                ChatEntry::Question { text } => {
                    open = Some(PastTurn {
                        question: text.clone(),
                        ..PastTurn::default()
                    });
                }
                ChatEntry::ToolCall { paths, .. } => {
                    if let Some(turn) = open.as_mut() {
                        mention(turn, paths.iter().cloned());
                    }
                }
                ChatEntry::Answer { text, targets, .. } => {
                    if let Some(mut turn) = open.take() {
                        turn.answer.clone_from(text);
                        mention(&mut turn, targets.iter().map(|target| tilde(&target.path)));
                        turns.push(turn);
                    }
                }
                ChatEntry::Hypothesis { path, .. } => {
                    if let Some(turn) = turns.last_mut() {
                        mention(turn, [path.clone()]);
                    }
                }
                ChatEntry::Failure { .. } => open = None,
            }
        }
        let forgotten = turns.len().saturating_sub(PAST_TURNS);
        turns.split_off(forgotten)
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

fn mention(turn: &mut PastTurn, paths: impl IntoIterator<Item = String>) {
    for path in paths {
        if !turn.paths.contains(&path) {
            turn.paths.push(path);
        }
    }
}
