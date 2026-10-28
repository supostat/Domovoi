mod fixture;

use std::collections::BTreeSet;
use std::process::Command;

use domovoi_agent::{Agent, AgentAnswer, AgentContext, AgentLimits, FocusContext};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_inference::CancelFlag;
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use fixture::{Fixture, loaded_fake, say};
use serde_json::{Value, json};

const CONTEXT_LENGTH: u32 = 16_384;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: false,
    empty_think_in_tool_steps: false,
};
const CHFLAGS: &str = "/usr/bin/chflags";

fn tool_call(name: &str, arguments: &Value) -> String {
    say(&json!({"kind": "tool_call", "name": name, "arguments": arguments}))
}

fn final_answer(targets: &[String]) -> String {
    let mut answer = json!({"kind": "final", "answer": "Лучше удалить только это.", "hypotheses": []});
    if !targets.is_empty() {
        answer["targets"] = json!(targets);
    }
    say(&answer)
}

fn ask(context: &AgentContext<'_>, replies: Vec<String>) -> AgentAnswer {
    let mut backend = loaded_fake(replies);
    Agent::new(&mut backend, TEMPLATE, AgentLimits::for_context_length(CONTEXT_LENGTH))
        .ask(
            context,
            "Отметь к удалению только это",
            &CancelFlag::default(),
            &mut |_| {},
        )
        .expect("the agent answers")
}

fn nodes(answer: &AgentAnswer) -> Vec<NodeId> {
    answer.actions.iter().map(|action| action.node).collect()
}

fn inspect(fixture: &Fixture, relative: &str) -> String {
    tool_call("inspect_node", &json!({"path": fixture.path(relative)}))
}

#[test]
fn a_named_target_inside_an_inspected_folder_is_the_only_action() {
    let fixture = Fixture::with_folders(&[]);
    let debug = fixture.node("engine/target/debug");
    let answer = ask(
        &fixture.context(),
        vec![
            inspect(&fixture, "engine/target"),
            final_answer(&[fixture.path("engine/target/debug")]),
        ],
    );
    assert_eq!(
        nodes(&answer),
        vec![debug],
        "the target replaces the inspected build folder"
    );
    let action = &answer.actions[0];
    assert_eq!(
        action.rule_id, "cargo.target",
        "the rule of the claimed ancestor applies"
    );
    assert_eq!(action.allocated, fixture.index.node(debug).allocated);
}

#[test]
fn without_targets_the_actions_come_from_the_tool_results_as_before() {
    let fixture = Fixture::with_folders(&[]);
    let answer = ask(
        &fixture.context(),
        vec![inspect(&fixture, "engine/target"), final_answer(&[])],
    );
    assert_eq!(nodes(&answer), vec![fixture.node("engine/target")]);
}

#[test]
fn targets_outside_the_index_or_without_an_allowing_rule_make_no_action() {
    let fixture = Fixture::with_folders(&[]);
    let answer = ask(
        &fixture.context(),
        vec![
            inspect(&fixture, "engine/target"),
            final_answer(&["/nowhere/invented".to_owned(), fixture.path("notes")]),
        ],
    );
    assert!(
        answer.actions.is_empty(),
        "named targets replace the inference even when none of them is allowed: {:?}",
        answer.actions
    );
}

#[test]
fn a_target_and_its_ancestor_make_one_action_on_the_ancestor() {
    let fixture = Fixture::with_folders(&[]);
    let answer = ask(
        &fixture.context(),
        vec![final_answer(&[
            fixture.path("engine/target/debug"),
            fixture.path("engine/target"),
        ])],
    );
    assert_eq!(nodes(&answer), vec![fixture.node("engine/target")]);
}

struct Frozen {
    tree: TempTree,
}

impl Drop for Frozen {
    fn drop(&mut self) {
        let status = Command::new(CHFLAGS)
            .args(["nouchg"])
            .arg(self.tree.path().join("frozen/target"))
            .status()
            .expect("chflags starts");
        assert!(status.success(), "the frozen folder is released");
    }
}

#[test]
fn a_target_the_denylist_refuses_makes_no_action() {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.root().expect("the root opens");
    let frozen = root.directory("frozen").expect("a directory is created");
    frozen.file("Cargo.toml", 64, Fill::Byte(1)).expect("a file is written");
    frozen
        .directory("target")
        .expect("a directory is created")
        .file("frozen", 40_000, Fill::Noise(1))
        .expect("a file is written");
    let status = Command::new(CHFLAGS)
        .args(["uchg"])
        .arg(tree.path().join("frozen/target"))
        .status()
        .expect("chflags starts");
    assert!(status.success(), "the build folder is frozen");
    let frozen = Frozen { tree };
    let report = scan(frozen.tree.path(), &ScanOptions::default()).expect("the tree scans");
    let index = Index::build(
        &report,
        IndexRoot::for_path(frozen.tree.path()).expect("the root kind is known"),
        &Enrichment::default(),
    );
    let rules = RuleSet::for_volume(frozen.tree.path(), &ConfigOverrides::default()).expect("the rules are prepared");
    let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
    let totals = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new()).totals;
    let home = index.path(index.root());
    let context = AgentContext {
        index: &index,
        rules: &rules,
        denylist: &denylist,
        totals,
        home: &home,
        history: &[],
        focus: FocusContext::none(),
    };
    let target = home.join("frozen/target").to_string_lossy().into_owned();
    let node = index
        .find(&home.join("frozen/target"))
        .expect("the build folder is indexed");
    assert!(
        rules.claim_of(&index, node).is_some(),
        "the build folder is claimed, so only the denylist can refuse it"
    );
    assert!(denylist.verdict(&index, node).is_some(), "the frozen folder is denied");
    let answer = ask(&context, vec![final_answer(&[target])]);
    assert!(
        answer.actions.is_empty(),
        "the frozen build folder stays: {:?}",
        answer.actions
    );
}
