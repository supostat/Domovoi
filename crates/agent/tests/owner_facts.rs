mod fixture;

use std::fs;

use domovoi_agent::{Agent, AgentAnswer, AgentLimits};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_inference::CancelFlag;
use domovoi_testkit::{TempTree, app_container, application_bundle};
use fixture::{Fixture, loaded_fake, say};
use serde_json::{Value, json};

const CONTEXT_LENGTH: u32 = 16_384;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: true,
    empty_think_in_tool_steps: false,
};
const LIBRARY: &str = "Users/tester/Library";

fn tool_call(name: &str, arguments: &Value) -> String {
    say(&json!({"kind": "tool_call", "name": name, "arguments": arguments}))
}

fn final_answer() -> String {
    say(&json!({"kind": "final", "answer": "Готов ответ.", "hypotheses": []}))
}

fn ask(fixture: &Fixture, replies: Vec<String>) -> AgentAnswer {
    let steps = u32::try_from(replies.len()).expect("a few steps");
    let mut backend = loaded_fake(replies);
    Agent::new(
        &mut backend,
        TEMPLATE,
        AgentLimits {
            max_steps: steps,
            ..AgentLimits::for_context_length(CONTEXT_LENGTH)
        },
    )
    .ask(
        &fixture.context(),
        "Чьи это папки?",
        &CancelFlag::default(),
        &mut |_| {},
    )
    .expect("the agent answers")
}

#[test]
fn the_assistant_sees_the_owner_of_a_folder_and_the_signal_behind_it() {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = fs::canonicalize(tree.path()).expect("the root resolves");
    application_bundle(&root.join("Applications"), "Tiny", "com.domovoi.tiny", &[]).expect("the application is built");
    app_container(
        &root.join(LIBRARY).join("Containers/com.domovoi.tiny"),
        "com.domovoi.tiny",
    )
    .expect("the container is built");
    app_container(
        &root.join(LIBRARY).join("Containers/com.example.renamed"),
        "com.domovoi.gone",
    )
    .expect("the container is built");
    let fixture = Fixture::on_volume(tree);
    let leftover = fixture.path(&format!("{LIBRARY}/Containers/com.example.renamed"));
    let installed_file = fixture.path(&format!("{LIBRARY}/Containers/com.domovoi.tiny/data.db"));
    let unowned = fixture.path("Applications");
    let answer = ask(
        &fixture,
        vec![
            tool_call("inspect_node", &json!({"path": leftover})),
            tool_call("inspect_node", &json!({"path": installed_file})),
            tool_call("inspect_node", &json!({"path": unowned})),
            final_answer(),
        ],
    );

    assert_eq!(
        answer.steps[0].result["owner"],
        json!({"bundle_id": "com.domovoi.gone", "installation": "missing", "evidence": "container_metadata"})
    );
    assert_eq!(
        answer.steps[1].result["owner"],
        json!({"bundle_id": "com.domovoi.tiny", "installation": "installed", "evidence": "container_metadata"})
    );
    assert_eq!(answer.steps[2].result["owner"], Value::Null);
}
