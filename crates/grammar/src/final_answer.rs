use serde_json::{Value, json};

use crate::model_hypothesis::ModelHypothesis;

const LONGEST_TARGET: u64 = 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct FinalAnswer {
    pub answer: String,
    pub hypotheses: Vec<ModelHypothesis>,
    pub targets: Vec<String>,
}

impl FinalAnswer {
    pub const MOST_TARGETS: usize = 5;

    pub fn target_schema() -> Value {
        json!({"type": "string", "minLength": 1, "maxLength": LONGEST_TARGET})
    }
}
