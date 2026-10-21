#![forbid(unsafe_code)]

mod agent;
mod agent_answer;
mod agent_context;
mod agent_error;
mod agent_limits;
#[cfg(feature = "claude-code")]
mod claude_answer;
#[cfg(feature = "claude-code")]
mod claude_code;
#[cfg(feature = "claude-code")]
mod claude_code_error;
#[cfg(feature = "claude-code")]
mod claude_process;
#[cfg(feature = "claude-code")]
mod claude_reply;
mod conversation;
mod focus_context;
mod home_path;
mod instructions;
mod level_name;
#[cfg(feature = "claude-code")]
mod mcp_connection;
#[cfg(feature = "claude-code")]
mod mcp_proxy;
#[cfg(feature = "claude-code")]
mod mcp_server;
#[cfg(feature = "claude-code")]
mod mcp_session;
mod past_turn;
#[cfg(feature = "claude-code")]
mod private_socket;
mod question_message;
mod suggested_action;
mod tool_outcome;
mod tool_runner;
mod tool_step;
mod untrusted_text;

pub use agent::Agent;
pub use agent_answer::AgentAnswer;
pub use agent_context::AgentContext;
pub use agent_error::AgentError;
pub use agent_limits::AgentLimits;
#[cfg(feature = "claude-code")]
pub use claude_answer::ClaudeAnswer;
#[cfg(feature = "claude-code")]
pub use claude_code::ClaudeCode;
#[cfg(feature = "claude-code")]
pub use claude_code_error::ClaudeCodeError;
pub use focus_context::FocusContext;
#[cfg(feature = "claude-code")]
pub use mcp_proxy::{MCP_PROXY_FLAG, relay_mcp_stdio};
#[cfg(feature = "claude-code")]
pub use mcp_session::McpSession;
pub use past_turn::PastTurn;
pub use suggested_action::SuggestedAction;
pub use tool_step::ToolStep;
