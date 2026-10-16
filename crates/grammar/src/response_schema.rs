use serde_json::{Map, Value, json};

use crate::compiled_schema::CompiledSchema;
use crate::final_answer::FinalAnswer;
use crate::gbnf_grammar::GbnfGrammar;
use crate::model_hypothesis::ModelHypothesis;
use crate::model_response::ModelResponse;
use crate::response_error::ResponseError;
use crate::strict_json::parse_strict;
use crate::tool::Tool;
use crate::tool_call::ToolCall;

const TOOL_CALL_KIND: &str = "tool_call";
const FINAL_KIND: &str = "final";
const FINAL_DEFINITION: &str = "final_answer";
const LONGEST_ANSWER: u64 = 1500;
const LONGEST_PATH: u64 = 1024;
const LONGEST_GUESS: u64 = 200;
const MOST_HYPOTHESES: u64 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct ResponseSchema {
    document: Value,
    compiled: CompiledSchema,
    allows_final: bool,
}

impl ResponseSchema {
    pub fn answers_and_tool_calls() -> Self {
        Self::build(true)
    }

    pub fn tool_calls_only() -> Self {
        Self::build(false)
    }

    pub fn document(&self) -> &Value {
        &self.document
    }

    pub fn allows_final(&self) -> bool {
        self.allows_final
    }

    pub fn grammar(&self) -> GbnfGrammar {
        self.compiled.grammar()
    }

    pub fn parse(&self, text: &str) -> Result<ModelResponse, ResponseError> {
        let value = parse_strict(text)?;
        self.compiled.validate(&value).map_err(ResponseError::Schema)?;
        let field = |name: &str| value.get(name).expect("the schema requires the field");
        Ok(match field("kind").as_str() {
            Some(TOOL_CALL_KIND) => ModelResponse::ToolCall(ToolCall {
                tool: Tool::from_name(field("name").as_str().expect("the schema makes the name a string"))
                    .expect("the schema lists only known tools"),
                arguments: field("arguments")
                    .as_object()
                    .expect("the schema makes arguments an object")
                    .clone(),
            }),
            Some(FINAL_KIND) => ModelResponse::Final(FinalAnswer {
                answer: field("answer")
                    .as_str()
                    .expect("the schema makes the answer a string")
                    .to_owned(),
                hypotheses: field("hypotheses")
                    .as_array()
                    .expect("the schema makes hypotheses an array")
                    .iter()
                    .map(hypothesis)
                    .collect(),
            }),
            other => unreachable!("the schema allows only known kinds, got {other:?}"),
        })
    }

    fn build(allows_final: bool) -> Self {
        let mut definitions = Map::new();
        for tool in Tool::ALL {
            definitions.insert(tool.name().to_owned(), tool_call_schema(tool));
        }
        if allows_final {
            definitions.insert(FINAL_DEFINITION.to_owned(), final_answer_schema());
        }
        let alternatives: Vec<Value> = definitions
            .keys()
            .map(|name| json!({"$ref": format!("#/$defs/{name}")}))
            .collect();
        let document = json!({"$defs": definitions, "oneOf": alternatives});
        let compiled = CompiledSchema::compile(&document).expect("the response schema stays inside the safe subset");
        Self {
            document,
            compiled,
            allows_final,
        }
    }
}

fn tool_call_schema(tool: Tool) -> Value {
    json!({
        "type": "object",
        "properties": {
            "kind": {"enum": [TOOL_CALL_KIND]},
            "name": {"enum": [tool.name()]},
            "arguments": tool.arguments_schema()
        },
        "required": ["kind", "name", "arguments"],
        "additionalProperties": false
    })
}

fn final_answer_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "kind": {"enum": [FINAL_KIND]},
            "answer": {"type": "string", "minLength": 1, "maxLength": LONGEST_ANSWER},
            "hypotheses": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "minLength": 1, "maxLength": LONGEST_PATH},
                        "guess": {"type": "string", "minLength": 1, "maxLength": LONGEST_GUESS},
                        "confidence": {"type": "number", "minimum": 0, "maximum": 1}
                    },
                    "required": ["path", "guess", "confidence"],
                    "additionalProperties": false
                },
                "maxItems": MOST_HYPOTHESES
            }
        },
        "required": ["kind", "answer", "hypotheses"],
        "additionalProperties": false
    })
}

fn hypothesis(value: &Value) -> ModelHypothesis {
    let text = |name: &str| {
        value[name]
            .as_str()
            .expect("the schema makes hypothesis fields strings")
            .to_owned()
    };
    ModelHypothesis {
        path: text("path"),
        guess: text("guess"),
        confidence: value["confidence"]
            .as_f64()
            .expect("the schema makes confidence a number"),
    }
}
