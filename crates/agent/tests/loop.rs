mod fixture;

use domovoi_agent::{Agent, AgentError, AgentLimits, ToolStep};
use domovoi_grammar::{ChatTemplate, PromptFamily, Tool};
use domovoi_index::SafetyLevel;
use domovoi_inference::{CancelFlag, InferenceBackend};
use fixture::{Fixture, loaded_fake, say};
use serde_json::{Value, json};

const CONTEXT_LENGTH: u32 = 16_384;
const EXTRA_FOLDERS: usize = 20;
const LISTED_CHILDREN: usize = 15;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: true,
    empty_think_in_tool_steps: false,
};

fn tool_call(name: &str, arguments: &Value) -> String {
    say(&json!({"kind": "tool_call", "name": name, "arguments": arguments}))
}

fn final_answer(answer: &str, hypotheses: &Value) -> String {
    say(&json!({"kind": "final", "answer": answer, "hypotheses": hypotheses}))
}

fn limits() -> AgentLimits {
    AgentLimits::for_context_length(CONTEXT_LENGTH)
}

#[test]
fn a_tool_result_feeds_the_next_step_and_actions_come_from_the_nodes_it_returned() {
    let fixture = Fixture::with_folders(&[]);
    let mut backend = loaded_fake(vec![
        tool_call(
            "query_index",
            &json!({"path_prefix": fixture.root_path(), "level": "safe", "limit": 5}),
        ),
        final_answer(
            "Результаты сборки Rust занимают больше всего: их можно пересоздать.",
            &json!([
                {"path": fixture.path("mystery"), "guess": "Похоже на остаток удалённого приложения", "confidence": 0.4},
                {"path": "/nowhere/invented", "guess": "Похоже на кэш", "confidence": 0.9}
            ]),
        ),
    ]);
    let mut observed: Vec<ToolStep> = Vec::new();
    let answer = Agent::new(&mut backend, TEMPLATE, limits())
        .ask(
            &fixture.context(),
            "Что можно безопасно удалить?",
            &CancelFlag::default(),
            &mut |step| {
                observed.push(step.clone());
            },
        )
        .expect("the agent answers");

    assert_eq!(answer.steps.len(), 1);
    assert_eq!(observed, answer.steps);
    let step = &answer.steps[0];
    assert_eq!(step.tool, Tool::QueryIndex);
    let target = fixture.path("engine/target");
    let listed: Vec<&str> = step.result["items"]
        .as_array()
        .expect("the result lists items")
        .iter()
        .map(|item| item["path"].as_str().expect("an item has a path"))
        .collect();
    assert!(listed.contains(&target.as_str()), "{listed:?}");
    assert!(backend.requests()[1].prompt.contains("cargo.target"));

    assert_eq!(
        answer.text,
        "Результаты сборки Rust занимают больше всего: их можно пересоздать."
    );
    assert_eq!(answer.hypotheses.len(), 1, "an invented path is dropped");
    assert_eq!(answer.hypotheses[0].path.to_string_lossy(), fixture.path("mystery"));
    let action = answer
        .actions
        .iter()
        .find(|action| action.node == fixture.node("engine/target"))
        .expect("the claimed build folder becomes an action");
    assert_eq!(action.rule_id, "cargo.target");
    assert_eq!(action.level, SafetyLevel::Safe);
    assert!(
        answer
            .actions
            .iter()
            .all(|action| action.level != SafetyLevel::CheckYourself)
    );
}

#[test]
fn the_answer_text_never_creates_actions() {
    let fixture = Fixture::with_folders(&[]);
    let target = fixture.path("engine/target");
    let mut backend = loaded_fake(vec![final_answer(
        &format!("Удалите {target} — это безопасно."),
        &json!([]),
    )]);
    let answer = Agent::new(&mut backend, TEMPLATE, limits())
        .ask(&fixture.context(), "Что удалить?", &CancelFlag::default(), &mut |_| {})
        .expect("the agent answers");
    assert!(answer.steps.is_empty());
    assert!(answer.actions.is_empty());
}

#[test]
fn rule_explain_inspect_node_and_summarize_answer_from_the_rules_and_the_index() {
    let fixture = Fixture::with_folders(&[]);
    let mut backend = loaded_fake(vec![
        tool_call("rule_explain", &json!({"id": "cargo.target"})),
        tool_call("inspect_node", &json!({"path": fixture.path("engine")})),
        tool_call("summarize", &json!({})),
        final_answer("Сборки Rust пересоздаются.", &json!([])),
    ]);
    let answer = Agent::new(
        &mut backend,
        TEMPLATE,
        AgentLimits {
            max_steps: 4,
            ..limits()
        },
    )
    .ask(
        &fixture.context(),
        "Расскажи про проект engine",
        &CancelFlag::default(),
        &mut |_| {},
    )
    .expect("the agent answers");

    let rule = fixture
        .rules
        .rules()
        .iter()
        .find(|rule| rule.id == "cargo.target")
        .expect("the rule exists");
    assert_eq!(answer.steps[0].result["explanation"], rule.explanation);
    assert_eq!(answer.steps[0].result["level"], "safe");
    let children: Vec<&str> = answer.steps[1].result["largest_children"]
        .as_array()
        .expect("the node lists its children")
        .iter()
        .map(|child| child["path"].as_str().expect("a child has a path"))
        .collect();
    assert_eq!(children.first().copied(), Some(fixture.path("engine/target").as_str()));
    let summary = &answer.steps[2].result;
    assert_eq!(summary["reclaimable"]["safe"], fixture.totals.safe);
    assert_eq!(
        summary["used_bytes"],
        fixture.index.node(fixture.index.root()).allocated
    );
    assert!(
        summary["rules"]
            .as_array()
            .expect("the summary lists rules")
            .iter()
            .any(|rule| rule["rule"] == "cargo.target")
    );
}

#[test]
fn unknown_rules_and_paths_answer_with_an_error_instead_of_failing_the_loop() {
    let fixture = Fixture::with_folders(&[]);
    let mut backend = loaded_fake(vec![
        tool_call("rule_explain", &json!({"id": "no.such-rule"})),
        tool_call("inspect_node", &json!({"path": "/nowhere"})),
        final_answer("Такого правила нет.", &json!([])),
    ]);
    let answer = Agent::new(&mut backend, TEMPLATE, limits())
        .ask(
            &fixture.context(),
            "Что за правило no.such-rule?",
            &CancelFlag::default(),
            &mut |_| {},
        )
        .expect("the agent answers");
    assert_eq!(answer.steps[0].result["error"], "правило не найдено");
    assert_eq!(answer.steps[1].result["error"], "узел не найден");
    assert!(answer.steps.iter().all(|step| step.nodes.is_empty()));
}

#[test]
fn inspect_node_lists_the_fifteen_largest_children_and_counts_them_all() {
    let extra: Vec<String> = (0..EXTRA_FOLDERS).map(|number| format!("extra-{number:02}")).collect();
    let names: Vec<&str> = extra.iter().map(String::as_str).collect();
    let fixture = Fixture::with_folders(&names);
    let mut backend = loaded_fake(vec![
        tool_call("inspect_node", &json!({"path": fixture.root_path()})),
        final_answer("Вот содержимое.", &json!([])),
    ]);
    let answer = Agent::new(&mut backend, TEMPLATE, limits())
        .ask(
            &fixture.context(),
            "Что лежит в домашней папке?",
            &CancelFlag::default(),
            &mut |_| {},
        )
        .expect("the agent answers");
    let root = &answer.steps[0].result;
    let total = fixture.index.children(fixture.index.root()).count();
    assert!(
        total > LISTED_CHILDREN,
        "the fixture has more children than one look shows"
    );
    assert_eq!(root["children_total"], total);
    assert_eq!(root["largest_children"].as_array().map(Vec::len), Some(LISTED_CHILDREN));
}

#[test]
fn the_grammar_forces_a_tool_first_when_asked_and_a_final_answer_on_the_last_step() {
    let fixture = Fixture::with_folders(&[]);
    let mut backend = loaded_fake(vec![
        tool_call("summarize", &json!({})),
        final_answer("Можно освободить немного места.", &json!([])),
    ]);
    Agent::new(
        &mut backend,
        TEMPLATE,
        AgentLimits {
            max_steps: 2,
            ..limits()
        },
    )
    .calling_a_tool_first()
    .ask(
        &fixture.context(),
        "Сколько можно освободить?",
        &CancelFlag::default(),
        &mut |_| {},
    )
    .expect("the agent answers");
    let grammars: Vec<&str> = backend
        .requests()
        .iter()
        .map(|request| request.grammar.as_deref().expect("every step is constrained"))
        .collect();
    assert_eq!(grammars.len(), 2);
    assert!(grammars[0].contains("tool_call") && !grammars[0].contains("final"));
    assert!(grammars[1].contains("final") && !grammars[1].contains("tool_call"));
}

#[test]
fn a_response_outside_the_schema_and_a_cancellation_are_reported() {
    let fixture = Fixture::with_folders(&[]);
    let mut backend = loaded_fake(vec![say(&json!({"kind": "final", "answer": ""}))]);
    let outcome = Agent::new(&mut backend, TEMPLATE, limits()).ask(
        &fixture.context(),
        "Что занимает место?",
        &CancelFlag::default(),
        &mut |_| {},
    );
    assert!(
        matches!(outcome, Err(AgentError::InvalidResponse { .. })),
        "{outcome:?}"
    );

    let mut backend = loaded_fake(vec![final_answer("Не успею.", &json!([]))]);
    let cancel = CancelFlag::default();
    cancel.raise();
    let outcome = Agent::new(&mut backend, TEMPLATE, limits()).ask(
        &fixture.context(),
        "Что занимает место?",
        &cancel,
        &mut |_| {},
    );
    assert!(matches!(outcome, Err(AgentError::Cancelled)), "{outcome:?}");
}

#[test]
fn old_tool_results_are_trimmed_when_the_context_runs_short() {
    let fixture = Fixture::with_folders(&[]);
    let probe = Fixture::with_folders(&[]);
    let mut measuring = loaded_fake(vec![final_answer("Ок.", &json!([]))]);
    Agent::new(&mut measuring, TEMPLATE, limits())
        .ask(
            &probe.context(),
            "Что занимает место?",
            &CancelFlag::default(),
            &mut |_| {},
        )
        .expect("the agent answers");
    let opening = measuring.requests()[0].prompt.clone();
    let opening_tokens = measuring
        .count_tokens(&opening)
        .expect("the fake backend counts tokens");
    let tight = AgentLimits {
        context_length: opening_tokens + 300 + limits().max_tokens_per_step,
        ..limits()
    };
    let mut backend = loaded_fake(vec![
        tool_call("query_index", &json!({"path_prefix": fixture.root_path(), "limit": 50})),
        final_answer("Ок.", &json!([])),
    ]);
    Agent::new(&mut backend, TEMPLATE, tight)
        .ask(
            &fixture.context(),
            "Что занимает место?",
            &CancelFlag::default(),
            &mut |_| {},
        )
        .expect("the agent answers within the trimmed context");
    let second = backend.requests()[1].prompt.clone();
    assert!(second.contains(r#"{"trimmed":true}"#), "{second}");
    let second_tokens = backend.count_tokens(&second).expect("the fake backend counts tokens");
    assert!(second_tokens + tight.max_tokens_per_step <= tight.context_length);
}
