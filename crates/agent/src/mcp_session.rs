use domovoi_grammar::{CompiledSchema, FinalAnswer, ModelHypothesis, Tool, ToolCall};
use serde_json::{Value, json};

use crate::agent_answer::AgentAnswer;
use crate::agent_context::AgentContext;
use crate::home_path::resolved;
use crate::tool_runner::ToolRunner;
use crate::tool_step::ToolStep;
use crate::untrusted_text::untrusted_json;

pub(crate) const SERVER_NAME: &str = "domovoi";
const NOTE_HYPOTHESIS: &str = "note_hypothesis";
const NOTE_HYPOTHESIS_DESCRIPTION: &str =
    "записать гипотезу о незнакомой папке или файле: полный путь, догадка «Похоже на…» и уверенность от 0 до 1";
const LATEST_PROTOCOL: &str = "2025-11-25";
const KNOWN_PROTOCOLS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const PARSE_ERROR: i64 = -32_700;
const INVALID_REQUEST: i64 = -32_600;
const METHOD_NOT_FOUND: i64 = -32_601;
const INVALID_PARAMS: i64 = -32_602;

pub struct McpSession<'a> {
    context: &'a AgentContext<'a>,
    runner: ToolRunner<'a>,
    steps: Vec<ToolStep>,
    hypotheses: Vec<ModelHypothesis>,
}

impl std::fmt::Debug for McpSession<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpSession")
            .field("steps", &self.steps)
            .field("hypotheses", &self.hypotheses)
            .finish_non_exhaustive()
    }
}

impl<'a> McpSession<'a> {
    pub fn new(context: &'a AgentContext<'a>) -> Self {
        Self {
            context,
            runner: ToolRunner::new(context),
            steps: Vec::new(),
            hypotheses: Vec::new(),
        }
    }

    pub fn respond(&mut self, message: &str) -> Option<String> {
        let Ok(request) = serde_json::from_str::<Value>(message) else {
            return Some(failure(&Value::Null, PARSE_ERROR, "сообщение — не JSON"));
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Some(failure(&id, INVALID_REQUEST, "нужен JSON-RPC 2.0"));
        }
        let method = request.get("method").and_then(Value::as_str)?;
        request.get("id")?;
        let params = request.get("params").unwrap_or(&Value::Null);
        let outcome = match method {
            "initialize" => Ok(initialized(params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_list()})),
            "tools/call" => self.called(params),
            _ => Err((METHOD_NOT_FOUND, format!("метод {method} не поддерживается"))),
        };
        Some(match outcome {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
            Err((code, message)) => failure(&id, code, &message),
        })
    }

    pub fn steps(&self) -> &[ToolStep] {
        &self.steps
    }

    pub fn answered(self, text: String) -> AgentAnswer {
        let answer = FinalAnswer {
            answer: text,
            hypotheses: self.hypotheses,
        };
        AgentAnswer::assembled(self.context, answer, self.steps)
    }

    fn called(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (INVALID_PARAMS, "нет имени инструмента".to_owned()))?;
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        if name == NOTE_HYPOTHESIS {
            return Ok(self.noted(&arguments));
        }
        let tool = Tool::from_name(name).ok_or_else(|| (INVALID_PARAMS, format!("неизвестный инструмент {name}")))?;
        let schema =
            CompiledSchema::compile(&tool.arguments_schema()).expect("tool schemas stay inside the safe subset");
        if let Err(violation) = schema.validate(&arguments) {
            return Ok(tool_result(&rejected_arguments(&violation.to_string()), true));
        }
        let call = ToolCall {
            tool,
            arguments: arguments
                .as_object()
                .cloned()
                .expect("the schema makes arguments an object"),
        };
        let outcome = self.runner.run(&call);
        let result = tool_result(&outcome.result, false);
        self.steps.push(ToolStep {
            tool,
            arguments: call.arguments,
            result: outcome.result,
            nodes: outcome.nodes,
        });
        Ok(result)
    }

    fn noted(&mut self, arguments: &Value) -> Value {
        let schema = CompiledSchema::compile(&ModelHypothesis::schema())
            .expect("the hypothesis schema stays inside the safe subset");
        if let Err(violation) = schema.validate(arguments) {
            return tool_result(&rejected_arguments(&violation.to_string()), true);
        }
        if self.hypotheses.len() == ModelHypothesis::MOST_PER_ANSWER {
            let limit =
                json!({"error": "гипотез на один ответ больше нельзя", "most": ModelHypothesis::MOST_PER_ANSWER});
            return tool_result(&limit, true);
        }
        let hypothesis = ModelHypothesis::from_validated(arguments);
        let path = resolved(&hypothesis.path, self.context.home, self.context.index);
        if self.context.index.find(&path).is_none() {
            return tool_result(&json!({"error": "узел не найден", "path": hypothesis.path}), true);
        }
        self.hypotheses.push(hypothesis);
        tool_result(&json!({"noted": true}), false)
    }
}

fn initialized(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .filter(|version| KNOWN_PROTOCOLS.contains(version))
        .unwrap_or(LATEST_PROTOCOL);
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION")}
    })
}

fn tool_list() -> Vec<Value> {
    let mut tools: Vec<Value> = Tool::ALL
        .into_iter()
        .map(|tool| {
            json!({
                "name": tool.name(),
                "description": tool.description(),
                "inputSchema": tool.arguments_schema()
            })
        })
        .collect();
    tools.push(json!({
        "name": NOTE_HYPOTHESIS,
        "description": NOTE_HYPOTHESIS_DESCRIPTION,
        "inputSchema": ModelHypothesis::schema()
    }));
    tools
}

fn rejected_arguments(violation: &str) -> Value {
    json!({"error": "аргументы не подходят к схеме", "detail": violation})
}

fn tool_result(result: &Value, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": untrusted_json(result)}], "isError": is_error})
}

fn failure(id: &Value, code: i64, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}
