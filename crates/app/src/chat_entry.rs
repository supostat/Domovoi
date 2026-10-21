use serde::Deserialize;

use crate::answer_item::AnswerItem;
use crate::answer_target::AnswerTarget;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatEntry {
    Question {
        text: String,
    },
    ToolCall {
        tool: String,
        arguments: String,
        found: Option<u64>,
        #[serde(skip)]
        paths: Vec<String>,
    },
    Answer {
        text: String,
        #[serde(default)]
        items: Vec<AnswerItem>,
        #[serde(skip)]
        targets: Vec<AnswerTarget>,
    },
    Hypothesis {
        path: String,
        text: String,
    },
    Failure {
        text: String,
    },
}

impl ChatEntry {
    pub fn answer_total(&self) -> u64 {
        match self {
            Self::Answer { items, .. } => items.iter().map(|item| item.bytes).sum(),
            Self::Question { .. } | Self::ToolCall { .. } | Self::Hypothesis { .. } | Self::Failure { .. } => 0,
        }
    }
}
