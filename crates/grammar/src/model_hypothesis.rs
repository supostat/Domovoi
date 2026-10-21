use serde_json::{Value, json};

const LONGEST_PATH: u64 = 1024;
const LONGEST_GUESS: u64 = 200;

#[derive(Clone, Debug, PartialEq)]
pub struct ModelHypothesis {
    pub path: String,
    pub guess: String,
    pub confidence: f64,
}

impl ModelHypothesis {
    pub const MOST_PER_ANSWER: usize = 5;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "minLength": 1, "maxLength": LONGEST_PATH},
                "guess": {"type": "string", "minLength": 1, "maxLength": LONGEST_GUESS},
                "confidence": {"type": "number", "minimum": 0, "maximum": 1}
            },
            "required": ["path", "guess", "confidence"],
            "additionalProperties": false
        })
    }

    pub fn from_validated(value: &Value) -> Self {
        let text = |name: &str| {
            value[name]
                .as_str()
                .expect("the schema makes hypothesis fields strings")
                .to_owned()
        };
        Self {
            path: text("path"),
            guess: text("guess"),
            confidence: value["confidence"]
                .as_f64()
                .expect("the schema makes confidence a number"),
        }
    }
}
