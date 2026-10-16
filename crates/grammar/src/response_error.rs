use std::fmt;

use crate::schema_violation::SchemaViolation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResponseError {
    Json(String),
    DuplicateKey(String),
    Schema(SchemaViolation),
}

impl fmt::Display for ResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(reason) => write!(formatter, "the response is not JSON: {reason}"),
            Self::DuplicateKey(key) => write!(formatter, "the response repeats the key {key}"),
            Self::Schema(violation) => write!(formatter, "the response breaks the schema at {violation}"),
        }
    }
}

impl std::error::Error for ResponseError {}
