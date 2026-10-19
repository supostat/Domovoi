mod fixture;

use domovoi_agent::{Agent, AgentAnswer, AgentLimits};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_inference::CancelFlag;
use fixture::{Fixture, loaded_fake, say};
use serde_json::{Value, json};

const CONTEXT_LENGTH: u32 = 16_384;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: true,
    empty_think_in_tool_steps: false,
};

fn tool_call(name: &str, arguments: &Value) -> String {
    say(&json!({"kind": "tool_call", "name": name, "arguments": arguments}))
}

fn final_answer(hypotheses: &Value) -> String {
    say(&json!({"kind": "final", "answer": "Готов ответ.", "hypotheses": hypotheses}))
}

fn ask(fixture: &Fixture, replies: Vec<String>) -> (AgentAnswer, String) {
    let steps = u32::try_from(replies.len()).expect("a few steps");
    let mut backend = loaded_fake(replies);
    let answer = Agent::new(
        &mut backend,
        TEMPLATE,
        AgentLimits {
            max_steps: steps,
            ..AgentLimits::for_context_length(CONTEXT_LENGTH)
        },
    )
    .ask(
        &fixture.context(),
        "Что в домашней папке?",
        &CancelFlag::default(),
        &mut |_| {},
    )
    .expect("the agent answers");
    let opening = backend.requests()[0].prompt.clone();
    (answer, opening)
}

fn listed_paths(result: &Value, field: &str) -> Vec<String> {
    result[field]
        .as_array()
        .unwrap_or_else(|| panic!("the result lists {field}: {result}"))
        .iter()
        .map(|item| item["path"].as_str().expect("an item has a path").to_owned())
        .collect()
}

#[test]
fn home_relative_paths_reach_the_index_and_a_rule_names_its_paths_and_findings() {
    let fixture = Fixture::with_folders(&[]);
    let (answer, opening) = ask(
        &fixture,
        vec![
            tool_call("inspect_node", &json!({"path": "~/engine"})),
            tool_call("query_index", &json!({"path_prefix": "~/engine", "level": "safe"})),
            tool_call("rule_explain", &json!({"id": "cargo.target"})),
            final_answer(&json!([
                {"path": "~/mystery", "guess": "Похоже на данные удалённого приложения", "confidence": 0.4}
            ])),
        ],
    );
    let home = fixture.root_path();
    assert!(
        opening.contains(&format!("\"home\":\"{home}\"")),
        "the index summary names home: {opening}"
    );

    assert_eq!(answer.steps[0].result["path"], fixture.path("engine"));
    assert!(listed_paths(&answer.steps[1].result, "items").contains(&fixture.path("engine/target")));

    let rule = &answer.steps[2].result;
    assert_eq!(rule["paths"], json!(["**/target|Cargo.toml"]));
    assert_eq!(listed_paths(rule, "found"), [fixture.path("engine/target")]);
    assert_eq!(rule["found_total"], 1);
    assert_eq!(
        answer.steps[2].nodes,
        [fixture.node("engine/target")],
        "the rule's findings are nodes the answer may act on"
    );

    assert_eq!(answer.hypotheses.len(), 1);
    assert_eq!(answer.hypotheses[0].path.to_string_lossy(), fixture.path("mystery"));
}

#[test]
fn an_empty_query_tells_whether_its_prefix_exists() {
    let fixture = Fixture::with_folders(&[]);
    let (answer, _) = ask(
        &fixture,
        vec![
            tool_call(
                "query_index",
                &json!({"path_prefix": "/Users/igor/.pnpm", "level": "safe"}),
            ),
            tool_call("query_index", &json!({"path_prefix": "~/notes", "level": "safe"})),
            final_answer(&json!([])),
        ],
    );
    let missing = &answer.steps[0].result;
    assert_eq!(missing["total"], 0);
    assert_eq!(missing["prefix_found"], false);
    let filtered_out = &answer.steps[1].result;
    assert_eq!(filtered_out["total"], 0);
    assert_eq!(filtered_out["prefix_found"], true);
}
