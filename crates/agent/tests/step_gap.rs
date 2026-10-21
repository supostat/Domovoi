use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File};
use std::os::fd::OwnedFd;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use domovoi_agent::{Agent, AgentContext, AgentLimits, FocusContext};
use domovoi_apfs_sys::{EntryAttributes, ObjectKind};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_inference::{BackendError, CancelFlag, FakeBackend, InferenceBackend, PRESSURE_IDLE_GRACE};
use domovoi_inference_proto::{GenerateRequest, GenerationStats, ModelClass};
use domovoi_rules::{
    CaseRule, ConfigOverrides, Denylist, LevelTotals, Recommendations, RuleSet, SimulatorRuntimes, rule_table,
};
use domovoi_scanner::{Node, NodeState, ScanReport};
use domovoi_testkit::TempTree;
use serde_json::{Value, json};

const UNITS: usize = 1_000;
const OBJECTS_PER_UNIT: usize = 500;
const OBJECT_SIZE: u64 = 4_096;
const CREATED: u64 = 1_700_000_000;
const ATTEMPTS: usize = 3;
const CONTEXT_LENGTH: u32 = 16_384;
const STEPS: [&str; 5] = ["question", "query_index", "inspect_node", "rule_explain", "summarize"];
const TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: true,
    empty_think_in_tool_steps: false,
};

static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(10_000);

struct NoSimulators;

impl SimulatorRuntimes for NoSimulators {
    fn device_runtime_missing(&self, _device: &Path) -> bool {
        false
    }
}

fn attributes(kind: ObjectKind) -> EntryAttributes {
    let file_id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let created = UNIX_EPOCH + Duration::from_secs(CREATED);
    let file = kind != ObjectKind::Directory;
    EntryAttributes {
        kind,
        flags: 0,
        file_id,
        created,
        modified: created,
        added: Some(created),
        mount_status: (!file).then_some(0),
        link_count: file.then_some(1),
        total_size: file.then_some(OBJECT_SIZE),
        alloc_size: file.then_some(OBJECT_SIZE),
        data_alloc_size: file.then_some(OBJECT_SIZE),
        private_size: None,
        link_id: Some(file_id),
        real_dev: Some(1),
        clone_id: Some(file_id),
        ext_flags: Some(0),
        clone_refcount: file.then_some(1),
    }
}

fn file(name: &str) -> Node {
    Node {
        name: OsString::from(name),
        attributes: Some(attributes(ObjectKind::File)),
        last_used: None,
        state: NodeState::Leaf,
        children: Vec::new(),
    }
}

fn directory(name: &str, children: Vec<Node>) -> Node {
    Node {
        name: OsString::from(name),
        attributes: Some(attributes(ObjectKind::Directory)),
        last_used: None,
        state: NodeState::Listed,
        children,
    }
}

fn engine() -> Node {
    let units = (0..UNITS)
        .map(|unit| {
            let objects = (0..OBJECTS_PER_UNIT)
                .map(|object| file(&format!("object-{object}")))
                .collect();
            directory(&format!("unit-{unit}"), objects)
        })
        .collect();
    directory("engine", vec![file("Cargo.toml"), directory("target", units)])
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallKind {
    Generate,
    CountTokens,
}

struct Call {
    kind: CallKind,
    started: Instant,
    finished: Instant,
}

struct RecordingBackend {
    inner: FakeBackend,
    calls: Vec<Call>,
}

impl RecordingBackend {
    fn record(&mut self, kind: CallKind, started: Instant) {
        self.calls.push(Call {
            kind,
            started,
            finished: Instant::now(),
        });
    }

    fn gaps_by_step(&self) -> [Duration; STEPS.len()] {
        let mut gaps = [Duration::ZERO; STEPS.len()];
        let mut generations = 0;
        for pair in self.calls.windows(2) {
            if pair[0].kind == CallKind::Generate {
                generations += 1;
            }
            let gap = pair[1].started.duration_since(pair[0].finished);
            gaps[generations] = gaps[generations].max(gap);
        }
        gaps
    }
}

impl InferenceBackend for RecordingBackend {
    fn load(
        &mut self,
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
        cancel: &CancelFlag,
    ) -> Result<(), BackendError> {
        self.inner.load(weights, class, context_length, cancel)
    }

    fn unload(&mut self) {
        self.inner.unload();
    }

    fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }

    fn generate(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        let started = Instant::now();
        let outcome = self.inner.generate(request, cancel, on_token);
        self.record(CallKind::Generate, started);
        outcome
    }

    fn count_tokens(&mut self, text: &str) -> Result<u32, BackendError> {
        let started = Instant::now();
        let outcome = self.inner.count_tokens(text);
        self.record(CallKind::CountTokens, started);
        outcome
    }
}

struct Volume {
    tree: TempTree,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    totals: LevelTotals,
}

impl Volume {
    fn new() -> Self {
        let tree = TempTree::new().expect("a temp tree is created");
        let report = ScanReport {
            root: Node {
                name: tree.path().as_os_str().to_owned(),
                attributes: None,
                last_used: None,
                state: NodeState::Listed,
                children: vec![engine()],
            },
            files: 0,
            bytes: 0,
        };
        let index = Index::build(
            &report,
            IndexRoot::for_path(tree.path()).expect("the root kind is known"),
            &Enrichment::default(),
        );
        let rules = RuleSet::new(
            rule_table(&ConfigOverrides::default()),
            CaseRule::Insensitive,
            Box::new(NoSimulators),
        );
        let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
        let totals = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new()).totals;
        Self {
            tree,
            index,
            rules,
            denylist,
            totals,
        }
    }

    fn context(&self) -> AgentContext<'_> {
        AgentContext {
            index: &self.index,
            rules: &self.rules,
            denylist: &self.denylist,
            totals: self.totals,
            home: self.tree.path(),
            history: &[],
            focus: FocusContext::none(),
        }
    }

    fn largest_folder(&self) -> String {
        self.tree.path().join("engine/target").to_string_lossy().into_owned()
    }

    fn weights(&self) -> OwnedFd {
        let path = self.tree.path().join("weights.bin");
        fs::write(&path, b"GGUF").expect("the weights are written");
        File::open(&path).expect("the weights open").into()
    }

    fn backend(&self) -> RecordingBackend {
        let say = |response: Value| format!("token {response}");
        let tool_call =
            |name: &str, arguments: Value| say(json!({"kind": "tool_call", "name": name, "arguments": arguments}));
        let mut inner = FakeBackend::replying(vec![
            tool_call("query_index", json!({"level": "check_yourself", "limit": 10})),
            tool_call("inspect_node", json!({"path": self.largest_folder()})),
            tool_call("rule_explain", json!({"id": "cargo.target"})),
            tool_call("summarize", json!({})),
            say(json!({"kind": "final", "answer": "Сборки Rust пересоздаются.", "hypotheses": []})),
        ]);
        inner
            .load(
                self.weights(),
                ModelClass::Compact,
                CONTEXT_LENGTH,
                &CancelFlag::default(),
            )
            .expect("the fake model loads");
        RecordingBackend {
            inner,
            calls: Vec::new(),
        }
    }

    fn answer_gaps(&self) -> [Duration; STEPS.len()] {
        let mut backend = self.backend();
        let limits = AgentLimits {
            max_steps: STEPS.len() as u32,
            ..AgentLimits::for_context_length(CONTEXT_LENGTH)
        };
        let answer = Agent::new(&mut backend, TEMPLATE, limits)
            .calling_a_tool_first()
            .ask(
                &self.context(),
                "Что занимает место?",
                &CancelFlag::default(),
                &mut |_| {},
            )
            .expect("the agent answers");
        assert_eq!(answer.steps.len(), STEPS.len() - 1, "every tool ran once");
        backend.gaps_by_step()
    }
}

#[test]
fn the_longest_pause_between_model_calls_of_one_answer_fits_half_the_pressure_grace() {
    let volume = Volume::new();
    let mut fastest = [Duration::MAX; STEPS.len()];
    for _ in 0..ATTEMPTS {
        for (best, gap) in fastest.iter_mut().zip(volume.answer_gaps()) {
            *best = (*best).min(gap);
        }
    }
    let table: Vec<(&str, Duration)> = STEPS.into_iter().zip(fastest).collect();
    let longest = fastest.into_iter().max().expect("an answer has steps");
    assert!(
        longest * 2 <= PRESSURE_IDLE_GRACE,
        "pauses by step {table:?}: the longest {longest:?} exceeds half of {PRESSURE_IDLE_GRACE:?}"
    );
}
