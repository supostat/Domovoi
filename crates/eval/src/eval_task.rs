use domovoi_agent::PastTurn;
use domovoi_grammar::Tool;
use serde::{Deserialize, Deserializer};

use crate::earlier_turn::EarlierTurn;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalTask {
    pub id: String,
    #[serde(default)]
    pub before: Vec<EarlierTurn>,
    pub question: String,
    #[serde(deserialize_with = "tool_named")]
    pub expected_tool: Option<Tool>,
    pub needs_tool: bool,
    pub reference: String,
}

impl EvalTask {
    pub fn history(&self) -> Vec<PastTurn> {
        self.before.iter().map(EarlierTurn::past_turn).collect()
    }
}

fn tool_named<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Tool>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|name| Tool::from_name(&name).ok_or_else(|| serde::de::Error::custom(format!("unknown tool {name:?}"))))
        .transpose()
}
