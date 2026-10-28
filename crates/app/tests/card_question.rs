use std::collections::BTreeSet;
use std::fs::File;
use std::path::Path;
use std::time::SystemTime;

use domovoi_agent::{Agent, AgentContext, AgentLimits, FocusContext};
use domovoi_app::{CardModel, Provenance, RecommendationsModel, SafetyBadge, card_focus, strings};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_index::{Enrichment, Index, IndexRoot, SafetyLevel};
use domovoi_inference::{CancelFlag, FakeBackend, InferenceBackend};
use domovoi_inference_proto::ModelClass;
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use serde_json::json;

const TINY_MODEL: &str = "../../assets/test/tiny-random.gguf";
const FAKE_CONTEXT: u32 = 2048;
const CONTEXT_LENGTH: u32 = 16_384;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: false,
    empty_think_in_tool_steps: false,
};
const RULE: &str = "cargo.target";
const QUESTION_MARK: &str = "Вопрос: ";

struct Scanned {
    _tree: TempTree,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    recommendations: Recommendations,
}

fn scanned() -> Scanned {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.root().expect("the root opens");
    let engine = root.directory("engine").expect("a directory is created");
    engine.file("Cargo.toml", 64, Fill::Byte(1)).expect("a file is written");
    engine
        .directory("target")
        .expect("a directory is created")
        .directory("debug")
        .expect("a directory is created")
        .file("engine", 90_000, Fill::Noise(1))
        .expect("a file is written");
    let report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
    let index = Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("the root kind is known"),
        &Enrichment::default(),
    );
    let rules = RuleSet::for_volume(tree.path(), &ConfigOverrides::default()).expect("the rules are prepared");
    let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
    let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());
    Scanned {
        _tree: tree,
        index,
        rules,
        denylist,
        recommendations,
    }
}

fn rule_card(scanned: &Scanned) -> CardModel {
    RecommendationsModel::build(
        &scanned.index,
        &scanned.rules,
        &scanned.recommendations,
        SystemTime::now(),
    )
    .cards
    .into_iter()
    .find(|card| card.provenance == Provenance::Rule(RULE.to_owned()))
    .expect("the build folder makes a card")
}

fn loaded_fake(replies: Vec<String>) -> FakeBackend {
    let mut backend = FakeBackend::replying(replies);
    let weights = File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(TINY_MODEL)).expect("the weights open");
    backend
        .load(
            weights.into(),
            ModelClass::Compact,
            FAKE_CONTEXT,
            &CancelFlag::default(),
        )
        .expect("the fake backend loads");
    backend
}

fn say(response: &serde_json::Value) -> String {
    format!("token {response}")
}

#[test]
fn a_rule_card_names_its_rule_level_paths_and_size() {
    let scanned = scanned();
    let card = rule_card(&scanned);
    let focus = card_focus(&card);
    assert_eq!(focus.rule.as_deref(), Some(RULE));
    assert_eq!(focus.title, card.title);
    assert_eq!(focus.level, SafetyLevel::Safe);
    assert_eq!(focus.paths, card.paths);
    assert!(
        focus.paths.iter().any(|path| path.ends_with("target")),
        "{:?}",
        focus.paths
    );
    assert_eq!(focus.bytes, card.amount);
}

#[test]
fn a_hypothesis_card_has_no_rule_and_asks_to_be_checked() {
    let card = CardModel {
        title: "com.tinyapp.helper".to_owned(),
        paths: vec!["~/Library/Application Support/com.tinyapp.helper".to_owned()],
        path_reasons: Vec::new(),
        explanation: "Похоже на остаток удалённого приложения".to_owned(),
        level: SafetyBadge::CheckYourself,
        provenance: Provenance::Hypothesis,
        amount: 1_200_000_000,
        held_by_snapshots: 0,
        held_by_clones: 0,
        held_by_hard_links: 0,
        owner_running: None,
        blocked: None,
        vendor_command: None,
        age: None,
        selected: false,
        targets: Vec::new(),
    };
    let focus = card_focus(&card);
    assert_eq!(focus.rule, None);
    assert_eq!(focus.level, SafetyLevel::CheckYourself);
    assert_eq!(focus.paths, card.paths);
}

#[test]
fn the_real_agent_receives_the_card_as_data_of_the_overview() {
    let scanned = scanned();
    let card = rule_card(&scanned);
    let focus = FocusContext {
        card: Some(card_focus(&card)),
        ..FocusContext::default()
    };
    let home = scanned.index.path(scanned.index.root());
    let context = AgentContext {
        index: &scanned.index,
        rules: &scanned.rules,
        denylist: &scanned.denylist,
        totals: scanned.recommendations.totals,
        home: &home,
        history: &[],
        focus: &focus,
    };
    let mut backend = loaded_fake(vec![
        say(&json!({"kind": "tool_call", "name": "rule_explain", "arguments": {"id": RULE}})),
        say(&json!({"kind": "final", "answer": "Сборки Rust пересоздаются.", "hypotheses": []})),
    ]);
    let answer = Agent::new(&mut backend, TEMPLATE, AgentLimits::for_context_length(CONTEXT_LENGTH))
        .ask(&context, strings::CARD_QUESTION, &CancelFlag::default(), &mut |_| {})
        .expect("the agent answers");
    assert_eq!(answer.steps.len(), 1, "the agent explains the rule first");
    let prompt = &backend.requests()[0].prompt;
    let question_at = prompt.rfind(QUESTION_MARK).expect("the prompt carries the question");
    let (overview, question) = prompt.split_at(question_at);
    assert!(overview.contains(r#""card":"#), "{prompt}");
    assert!(overview.contains(&format!(r#""rule":"{RULE}""#)), "{prompt}");
    for path in &card.paths {
        assert!(
            overview.contains(path.as_str()),
            "{path} is data of the overview: {prompt}"
        );
        assert!(
            !question.contains(path.as_str()),
            "the question names no path: {question}"
        );
    }
    assert!(question.contains(strings::CARD_QUESTION), "{question}");
}
