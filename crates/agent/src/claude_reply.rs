use std::process::Output;

use serde_json::Value;

use crate::claude_code_error::ClaudeCodeError;

const SUCCESS: &str = "success";
const LONGEST_DETAIL: usize = 500;

#[derive(Debug)]
pub(crate) struct ClaudeReply {
    pub(crate) result: String,
    pub(crate) session_id: String,
    pub(crate) structured: Option<Value>,
}

impl ClaudeReply {
    pub(crate) fn parse(output: &Output) -> Result<Self, ClaudeCodeError> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let Ok(reply) = serde_json::from_str::<Value>(stdout.trim()) else {
            return Err(if output.status.success() {
                ClaudeCodeError::Output(tail(&stdout))
            } else {
                ClaudeCodeError::Failed(format!(
                    "{}: {}",
                    output.status,
                    tail(&String::from_utf8_lossy(&output.stderr))
                ))
            });
        };
        let result = reply.get("result").and_then(Value::as_str);
        let failed = reply.get("is_error").and_then(Value::as_bool) != Some(false)
            || reply.get("subtype").and_then(Value::as_str) != Some(SUCCESS)
            || !output.status.success();
        if failed {
            let subtype = reply.get("subtype").and_then(Value::as_str).unwrap_or("unknown");
            return Err(ClaudeCodeError::Failed(format!(
                "{}, {subtype}: {}",
                output.status,
                tail(result.unwrap_or_default())
            )));
        }
        let text = |name: &str, value: Option<&str>| {
            value
                .map(str::to_owned)
                .ok_or_else(|| ClaudeCodeError::Output(format!("no {name} in the reply")))
        };
        Ok(Self {
            result: text("result", result)?,
            session_id: text("session_id", reply.get("session_id").and_then(Value::as_str))?,
            structured: reply.get("structured_output").cloned(),
        })
    }
}

fn tail(text: &str) -> String {
    let trimmed = text.trim();
    let skipped = trimmed.chars().count().saturating_sub(LONGEST_DETAIL);
    trimmed.chars().skip(skipped).collect()
}
