use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use domovoi_agent::{Agent, AgentAnswer, AgentContext, AgentLimits, FocusContext};
use domovoi_app::{AnswerItem, AnswerTarget, ChatEntry, ChatModel};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_inference::{CancelFlag, FakeBackend, InferenceBackend};
use domovoi_inference_proto::ModelClass;
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use serde_json::{Value, json};

const TINY_MODEL: &str = "../../assets/test/tiny-random.gguf";
const FAKE_CONTEXT: u32 = 2048;
const CONTEXT_LENGTH: u32 = 16_384;
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: false,
    empty_think_in_tool_steps: false,
};
const QUESTION: &str = "Отметь к удалению только debug";
const RULE: &str = "cargo.target";

struct Scanned {
    _tree: TempTree,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    recommendations: Recommendations,
    home: PathBuf,
}

impl Scanned {
    fn build() -> Self {
        let tree = TempTree::new().expect("a temp tree is created");
        let root = tree.root().expect("the root opens");
        let engine = root.directory("engine").expect("a directory is created");
        engine.file("Cargo.toml", 64, Fill::Byte(1)).expect("a file is written");
        let target = engine.directory("target").expect("a directory is created");
        target
            .directory("debug")
            .expect("a directory is created")
            .file("engine", 90_000, Fill::Noise(1))
            .expect("a file is written");
        target
            .directory("release")
            .expect("a directory is created")
            .file("engine", 60_000, Fill::Noise(2))
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
        let home = index.path(index.root());
        Self {
            _tree: tree,
            index,
            rules,
            denylist,
            recommendations,
            home,
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.home.join(relative)
    }

    fn allocated(&self, relative: &str) -> u64 {
        let node = self.index.find(&self.path(relative)).expect("the folder is indexed");
        self.index.node(node).allocated
    }

    fn ask(&self, replies: Vec<String>) -> AgentAnswer {
        let context = AgentContext {
            index: &self.index,
            rules: &self.rules,
            denylist: &self.denylist,
            totals: self.recommendations.totals,
            home: &self.home,
            history: &[],
            focus: FocusContext::none(),
        };
        let mut backend = loaded_fake(replies);
        Agent::new(&mut backend, TEMPLATE, AgentLimits::for_context_length(CONTEXT_LENGTH))
            .ask(&context, QUESTION, &CancelFlag::default(), &mut |_| {})
            .expect("the agent answers")
    }
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

fn say(response: &Value) -> String {
    format!("token {response}")
}

fn inspect(path: &Path) -> String {
    say(&json!({"kind": "tool_call", "name": "inspect_node", "arguments": {"path": path}}))
}

fn final_answer(targets: &[PathBuf]) -> String {
    let mut answer = json!({"kind": "final", "answer": "Лучше удалить только debug.", "hypotheses": []});
    if !targets.is_empty() {
        answer["targets"] = json!(targets);
    }
    say(&answer)
}

fn recorded(answer: &AgentAnswer) -> (Vec<AnswerItem>, Vec<AnswerTarget>) {
    let mut chat = ChatModel::default();
    chat.ask(QUESTION);
    chat.record_answer(answer);
    chat.entries
        .iter()
        .find_map(|entry| match entry {
            ChatEntry::Answer { items, targets, .. } => Some((items.clone(), targets.clone())),
            _ => None,
        })
        .expect("the chat records the answer")
}

#[test]
fn a_named_target_becomes_the_only_button_under_the_answer() {
    let scanned = Scanned::build();
    let answer = scanned.ask(vec![
        inspect(&scanned.path("engine/target")),
        final_answer(&[scanned.path("engine/target/debug")]),
    ]);
    let (items, targets) = recorded(&answer);
    assert_eq!(
        targets,
        vec![AnswerTarget {
            path: scanned.path("engine/target/debug"),
            rule_id: RULE.to_owned(),
        }],
        "the button points at the named folder, not at the inspected build folder"
    );
    assert_eq!(
        items.iter().map(|item| item.bytes).sum::<u64>(),
        scanned.allocated("engine/target/debug"),
        "the button counts only the named folder"
    );
    assert!(scanned.allocated("engine/target/debug") < scanned.allocated("engine/target"));
}

#[test]
fn without_targets_the_button_stays_on_the_inspected_folder() {
    let scanned = Scanned::build();
    let answer = scanned.ask(vec![inspect(&scanned.path("engine/target")), final_answer(&[])]);
    let (items, targets) = recorded(&answer);
    assert_eq!(
        targets,
        vec![AnswerTarget {
            path: scanned.path("engine/target"),
            rule_id: RULE.to_owned(),
        }]
    );
    assert_eq!(
        items.iter().map(|item| item.bytes).sum::<u64>(),
        scanned.allocated("engine/target")
    );
}
