use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ClaudeCodeError {
    Process(io::Error),
    Mcp(io::Error),
    Failed(String),
    Output(String),
    Cancelled,
}

impl fmt::Display for ClaudeCodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => write!(formatter, "the claude process failed: {error}"),
            Self::Mcp(error) => write!(formatter, "the MCP connection to claude failed: {error}"),
            Self::Failed(reason) => write!(formatter, "claude failed: {reason}"),
            Self::Output(reason) => write!(formatter, "claude answered in an unknown form: {reason}"),
            Self::Cancelled => formatter.write_str("the answer was cancelled"),
        }
    }
}

impl std::error::Error for ClaudeCodeError {}
