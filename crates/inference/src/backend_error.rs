use std::fmt;

use domovoi_inference_proto::{ErrorCode, Message};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendError {
    pub code: ErrorCode,
    pub description: String,
}

impl BackendError {
    pub fn new(code: ErrorCode, description: impl Into<String>) -> Self {
        Self {
            code,
            description: description.into(),
        }
    }

    pub fn into_message(self) -> Message {
        Message::Error {
            code: self.code,
            description: self.description,
        }
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.description)
    }
}

impl std::error::Error for BackendError {}
