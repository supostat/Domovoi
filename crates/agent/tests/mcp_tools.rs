mod fixture;

use domovoi_agent::{Agent, AgentLimits, McpSession};
use domovoi_grammar::{ChatTemplate, ModelHypothesis, PromptFamily, Tool};
use domovoi_index::SafetyLevel;
use domovoi_inference::CancelFlag;
use fixture::{Fixture, loaded_fake, say};
use serde_json::{Value, json};

const CONTEXT_LENGTH: u32 = 16_384;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: true,
    empty_think_in_tool_steps: false,
};
const PROTOCOL: &str = "2025-11-25";
const UNKNOWN_METHOD: i64 = -32_601;
const INVALID_PARAMS: i64 = -32_602;
const NOT_JSON: i64 = -32_700;

fn request(id: &Value, method: &str, params: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

fn call(id: u64, name: &str, arguments: &Value) -> String {
    request(&json!(id), "tools/call", &json!({"name": name, "arguments": arguments}))
}

fn reply(session: &mut McpSession<'_>, message: &str) -> Value {
    let text = session.respond(message).expect("a request gets a reply");
    serde_json::from_str(&text).expect("the reply is JSON")
}

fn tool_text(reply: &Value) -> Value {
    let text = reply["result"]["content"][0]["text"]
        .as_str()
        .expect("a tool result is text");
    serde_json::from_str(text).expect("the tool text is JSON")
}

#[test]
fn the_session_introduces_itself_and_lists_five_tools_with_their_schemas() {
    let fixture = Fixture::with_folders(&[]);
    let context = fixture.context();
    let mut session = McpSession::new(&context);
    let hello = reply(
        &mut session,
        &request(
            &json!(0),
            "initialize",
            &json!({"protocolVersion": PROTOCOL, "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}),
        ),
    );
    assert_eq!(hello["id"], 0);
    assert_eq!(hello["result"]["protocolVersion"], PROTOCOL);
    assert_eq!(hello["result"]["serverInfo"]["name"], "domovoi");
    assert!(hello["result"]["capabilities"]["tools"].is_object(), "{hello}");
    assert_eq!(
        session.respond(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
        None
    );

    let listed = reply(&mut session, &request(&json!(1), "tools/list", &json!({})));
    let tools = listed["result"]["tools"].as_array().expect("the tools are a list");
    let named = |name: &str| {
        tools
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("{name} is listed: {listed}"))
    };
    assert_eq!(tools.len(), Tool::ALL.len() + 1, "{listed}");
    for tool in Tool::ALL {
        let listed = named(tool.name());
        assert_eq!(listed["description"], tool.description());
        assert_eq!(listed["inputSchema"], tool.arguments_schema());
    }
    assert_eq!(named("note_hypothesis")["inputSchema"], ModelHypothesis::schema());
}

#[test]
fn every_tool_answers_what_the_agent_loop_gets_from_it() {
    let fixture = Fixture::with_folders(&[]);
    let calls = [
        (
            "query_index",
            json!({"path_prefix": fixture.root_path(), "level": "safe", "limit": 5}),
        ),
        ("inspect_node", json!({"path": fixture.path("engine")})),
        ("rule_explain", json!({"id": "cargo.target"})),
        ("summarize", json!({})),
    ];
    let mut replies: Vec<String> = calls
        .iter()
        .map(|(name, arguments)| say(&json!({"kind": "tool_call", "name": name, "arguments": arguments})))
        .collect();
    replies.push(say(&json!({"kind": "final", "answer": "Ок.", "hypotheses": []})));
    let mut backend = loaded_fake(replies);
    let limits = AgentLimits {
        max_steps: calls.len() as u32 + 1,
        ..AgentLimits::for_context_length(CONTEXT_LENGTH)
    };
    let local = Agent::new(&mut backend, TEMPLATE, limits)
        .ask(&fixture.context(), "Что на диске?", &CancelFlag::default(), &mut |_| {})
        .expect("the agent loop answers");

    let context = fixture.context();
    let mut session = McpSession::new(&context);
    for (number, (name, arguments)) in calls.iter().enumerate() {
        let called = reply(&mut session, &call(number as u64, name, arguments));
        assert_eq!(called["result"]["isError"], false, "{called}");
        assert_eq!(tool_text(&called), local.steps[number].result, "{name}");
    }
    assert_eq!(session.steps(), local.steps.as_slice());
}

#[test]
fn steps_nodes_and_hypotheses_gather_into_the_answer() {
    let fixture = Fixture::with_folders(&[]);
    let context = fixture.context();
    let mut session = McpSession::new(&context);
    reply(
        &mut session,
        &call(
            1,
            "query_index",
            &json!({"path_prefix": fixture.root_path(), "level": "safe"}),
        ),
    );
    let noted = reply(
        &mut session,
        &call(
            2,
            "note_hypothesis",
            &json!({"path": fixture.path("mystery"), "guess": "Похоже на архив старой программы", "confidence": 0.4}),
        ),
    );
    assert_eq!(tool_text(&noted), json!({"noted": true}));
    let invented = reply(
        &mut session,
        &call(
            3,
            "note_hypothesis",
            &json!({"path": "/nowhere/invented", "guess": "Похоже на кэш", "confidence": 0.9}),
        ),
    );
    assert_eq!(invented["result"]["isError"], true, "{invented}");

    let answer = session.answered("Сборку Rust можно пересоздать.".to_owned());
    assert_eq!(answer.text, "Сборку Rust можно пересоздать.");
    assert_eq!(answer.steps.len(), 1);
    assert_eq!(answer.steps[0].tool, Tool::QueryIndex);
    assert!(answer.steps[0].nodes.contains(&fixture.node("engine/target")));
    assert_eq!(answer.hypotheses.len(), 1, "an invented path is not noted");
    assert_eq!(answer.hypotheses[0].path.to_string_lossy(), fixture.path("mystery"));
    assert_eq!(answer.hypotheses[0].explanation, "Похоже на архив старой программы");
    let action = answer
        .actions
        .iter()
        .find(|action| action.node == fixture.node("engine/target"))
        .expect("the claimed build folder becomes an action");
    assert_eq!(action.level, SafetyLevel::Safe);
}

#[test]
fn protocol_mistakes_get_json_rpc_errors_and_bad_arguments_get_a_tool_error() {
    let fixture = Fixture::with_folders(&[]);
    let context = fixture.context();
    let mut session = McpSession::new(&context);
    let unknown_tool = reply(&mut session, &call(7, "delete_everything", &json!({})));
    assert_eq!(unknown_tool["id"], 7);
    assert_eq!(unknown_tool["error"]["code"], INVALID_PARAMS);

    let probe = reply(&mut session, &request(&json!("probe-1"), "server/discover", &json!({})));
    assert_eq!(probe["id"], "probe-1");
    assert_eq!(probe["error"]["code"], UNKNOWN_METHOD);

    let garbage = reply(&mut session, "{not json");
    assert_eq!(garbage["id"], Value::Null);
    assert_eq!(garbage["error"]["code"], NOT_JSON);

    let pathless = reply(&mut session, &call(8, "inspect_node", &json!({})));
    assert_eq!(pathless["result"]["isError"], true, "{pathless}");
    assert!(session.steps().is_empty(), "a rejected call makes no step");
}
