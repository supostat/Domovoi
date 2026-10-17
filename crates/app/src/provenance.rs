use serde::Deserialize;

use crate::strings::{MODEL_HYPOTHESIS, RULE_PROVENANCE};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Rule(String),
    Hypothesis,
}

impl Provenance {
    pub fn label(&self) -> String {
        match self {
            Self::Rule(id) => format!("{RULE_PROVENANCE} · {id}"),
            Self::Hypothesis => MODEL_HYPOTHESIS.to_owned(),
        }
    }
}
