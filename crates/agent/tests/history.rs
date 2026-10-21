mod fixture;

use std::path::PathBuf;

use domovoi_agent::{Agent, AgentContext, AgentLimits, FocusContext, PastTurn};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_inference::{CancelFlag, FakeBackend, InferenceBackend};
use fixture::{Fixture, loaded_fake, say};
use serde_json::{Value, json};

const CONTEXT_LENGTH: u32 = 16_384;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: true,
    empty_think_in_tool_steps: false,
};
const QUESTION: &str = "А внутри неё что?";
const OLDEST_QUESTION: &str = "Что в папке notes?";
const NEWER_QUESTION: &str = "А в папке mystery?";
const TRIMMED_RESULT: &str = r#"{"trimmed":true}"#;
const TRIMMED_EXCHANGE_ROOM: u32 = 300;

fn tool_call(name: &str, arguments: &Value) -> String {
    say(&json!({"kind": "tool_call", "name": name, "arguments": arguments}))
}

fn final_answer(answer: &str) -> String {
    say(&json!({"kind": "final", "answer": answer, "hypotheses": []}))
}

fn limits() -> AgentLimits {
    AgentLimits::for_context_length(CONTEXT_LENGTH)
}

fn past(fixture: &Fixture, question: &str, folder: &str) -> PastTurn {
    PastTurn {
        question: question.to_owned(),
        answer: format!("В {folder} лежат заметки."),
        paths: vec![fixture.path(folder)],
    }
}

fn with<'a>(fixture: &'a Fixture, history: &'a [PastTurn], focus: &'a FocusContext) -> AgentContext<'a> {
    AgentContext {
        history,
        focus,
        ..fixture.context()
    }
}

fn ask(backend: &mut FakeBackend, limits: AgentLimits, context: &AgentContext<'_>) {
    Agent::new(backend, TEMPLATE, limits)
        .ask(context, QUESTION, &CancelFlag::default(), &mut |_| {})
        .expect("the agent answers");
}

fn tokens(backend: &mut FakeBackend, request: usize) -> u32 {
    let prompt = backend.requests()[request].prompt.clone();
    backend.count_tokens(&prompt).expect("the fake backend counts tokens")
}

#[test]
fn a_follow_up_question_sees_the_previous_question_answer_and_path() {
    let fixture = Fixture::with_folders(&[]);
    let history = [past(&fixture, OLDEST_QUESTION, "notes")];
    let mut backend = loaded_fake(vec![final_answer("Ок.")]);
    ask(&mut backend, limits(), &with(&fixture, &history, FocusContext::none()));
    let prompt = &backend.requests()[0].prompt;
    let earlier = prompt
        .find(OLDEST_QUESTION)
        .expect("the previous question is in the prompt");
    let current = prompt.find(QUESTION).expect("the current question is in the prompt");
    assert!(earlier < current, "{prompt}");
    assert!(prompt.contains("В notes лежат заметки."), "{prompt}");
    assert!(
        prompt.contains(&format!(r#""last_paths":["{}"]"#, fixture.path("notes"))),
        "{prompt}"
    );
}

#[test]
fn the_overview_carries_the_focused_folder_its_largest_children_and_the_selected_cards() {
    let fixture = Fixture::with_folders(&[]);
    let focus = FocusContext {
        folder: Some(PathBuf::from(fixture.path("engine"))),
        selected: vec!["~/site/node_modules".to_owned()],
    };
    let mut backend = loaded_fake(vec![final_answer("Ок.")]);
    ask(&mut backend, limits(), &with(&fixture, &[], &focus));
    let prompt = &backend.requests()[0].prompt;
    let focus_at = prompt.find(r#""focus":"#).expect("the overview names the focus");
    let child_at = prompt[focus_at..]
        .find(&fixture.path("engine/target"))
        .expect("the focus lists its largest children");
    assert!(child_at > 0, "{prompt}");
    assert!(prompt.contains(r#""selected":["~/site/node_modules"]"#), "{prompt}");
}

#[test]
fn a_tight_window_trims_tool_results_before_it_forgets_the_oldest_turn() {
    let fixture = Fixture::with_folders(&[]);
    let history = [
        past(&fixture, OLDEST_QUESTION, "notes"),
        past(&fixture, NEWER_QUESTION, "mystery"),
    ];
    let context = with(&fixture, &history, FocusContext::none());
    let replies = || {
        vec![
            tool_call("query_index", &json!({"path_prefix": fixture.root_path(), "limit": 50})),
            final_answer("Ок."),
        ]
    };
    let mut measuring = loaded_fake(replies());
    ask(&mut measuring, limits(), &context);
    let opening = tokens(&mut measuring, 0);

    let roomy = AgentLimits {
        context_length: opening + TRIMMED_EXCHANGE_ROOM + limits().max_tokens_per_step,
        ..limits()
    };
    let mut backend = loaded_fake(replies());
    ask(&mut backend, roomy, &context);
    let second = &backend.requests()[1].prompt;
    assert!(second.contains(TRIMMED_RESULT), "{second}");
    assert!(
        second.contains(OLDEST_QUESTION),
        "a trimmed tool result is enough, so the oldest turn stays: {second}"
    );

    let newer_only = [history[1].clone()];
    let mut probe = loaded_fake(vec![final_answer("Ок.")]);
    ask(&mut probe, limits(), &with(&fixture, &newer_only, FocusContext::none()));
    let tight = AgentLimits {
        context_length: tokens(&mut probe, 0) + limits().max_tokens_per_step,
        ..limits()
    };
    let mut backend = loaded_fake(vec![final_answer("Ок.")]);
    ask(&mut backend, tight, &context);
    let first = &backend.requests()[0].prompt;
    assert!(
        !first.contains(OLDEST_QUESTION) && first.contains(NEWER_QUESTION),
        "without tool results to trim the oldest turn goes first: {first}"
    );
}

#[test]
fn without_history_the_conversation_opens_with_the_question_alone() {
    let fixture = Fixture::with_folders(&[]);
    let mut backend = loaded_fake(vec![final_answer("Ок.")]);
    ask(&mut backend, limits(), &fixture.context());
    let prompt = &backend.requests()[0].prompt;
    assert_eq!(prompt.matches("Вопрос:").count(), 1, "{prompt}");
    for key in [r#""focus""#, r#""selected""#, r#""last_paths""#] {
        assert!(!prompt.contains(key), "{key} stays out of an empty context: {prompt}");
    }
}
