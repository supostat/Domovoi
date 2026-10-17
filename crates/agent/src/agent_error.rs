use std::fmt;

use domovoi_grammar::ResponseError;
use domovoi_inference::BackendError;

#[derive(Debug)]
pub enum AgentError {
    Backend(BackendError),
    InvalidResponse { error: ResponseError, text: String },
    ContextOverflow { prompt_tokens: u32, context_length: u32 },
    Cancelled,
}

impl fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(error) => write!(formatter, "the model failed: {error}"),
            Self::InvalidResponse { error, .. } => write!(formatter, "the model answered outside the schema: {error}"),
            Self::ContextOverflow {
                prompt_tokens,
                context_length,
            } => write!(
                formatter,
                "a prompt of {prompt_tokens} tokens leaves no room for the answer in a context of {context_length}"
            ),
            Self::Cancelled => formatter.write_str("the answer was cancelled"),
        }
    }
}

impl std::error::Error for AgentError {}
