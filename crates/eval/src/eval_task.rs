use domovoi_grammar::Tool;
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalTask {
    pub id: String,
    pub question: String,
    #[serde(deserialize_with = "tool_named")]
    pub expected_tool: Option<Tool>,
    pub needs_tool: bool,
    pub reference: String,
}

fn tool_named<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Tool>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|name| Tool::from_name(&name).ok_or_else(|| serde::de::Error::custom(format!("unknown tool {name:?}"))))
        .transpose()
}
