#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use domovoi_agent::AgentContext;
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_inference::{CancelFlag, FakeBackend, InferenceBackend};
use domovoi_inference_proto::ModelClass;
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, LevelTotals, Recommendations, RuleSet};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use serde_json::Value;

const TINY_MODEL: &str = "../../assets/test/tiny-random.gguf";
const FAKE_CONTEXT: u32 = 2048;

pub(crate) struct Fixture {
    tree: TempTree,
    pub(crate) index: Index,
    pub(crate) rules: RuleSet,
    pub(crate) denylist: Denylist,
    pub(crate) totals: LevelTotals,
    pub(crate) home: PathBuf,
}

impl Fixture {
    pub(crate) fn with_folders(extra_folders: &[&str]) -> Self {
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
        let site = root.directory("site").expect("a directory is created");
        site.file("package.json", 64, Fill::Byte(2)).expect("a file is written");
        site.directory("node_modules")
            .expect("a directory is created")
            .directory("left-pad")
            .expect("a directory is created")
            .file("index.js", 40_000, Fill::Noise(2))
            .expect("a file is written");
        root.directory("notes")
            .expect("a directory is created")
            .file("todo.txt", 64, Fill::Byte(3))
            .expect("a file is written");
        root.directory("mystery")
            .expect("a directory is created")
            .file("blob.bin", 30_000, Fill::Noise(3))
            .expect("a file is written");
        for name in extra_folders {
            root.directory(name)
                .expect("a directory is created")
                .file("payload.bin", 2_000, Fill::Noise(4))
                .expect("a file is written");
        }
        let report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
        let index = Index::build(
            &report,
            IndexRoot::for_path(tree.path()).expect("the root kind is known"),
            &Enrichment::default(),
        );
        let rules = RuleSet::for_volume(tree.path(), &ConfigOverrides::default()).expect("the rules are prepared");
        let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
        let totals = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new()).totals;
        let home = index.path(index.root());
        Self {
            tree,
            index,
            rules,
            denylist,
            totals,
            home,
        }
    }

    pub(crate) fn context(&self) -> AgentContext<'_> {
        AgentContext {
            index: &self.index,
            rules: &self.rules,
            denylist: &self.denylist,
            totals: self.totals,
            home: &self.home,
        }
    }

    pub(crate) fn node(&self, relative: &str) -> NodeId {
        Path::new(relative)
            .components()
            .try_fold(self.index.root(), |node, component| {
                self.index.child_named(node, component.as_os_str())
            })
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    }

    pub(crate) fn path(&self, relative: &str) -> String {
        self.index.path(self.node(relative)).to_string_lossy().into_owned()
    }

    pub(crate) fn root_path(&self) -> String {
        self.index.path(self.index.root()).to_string_lossy().into_owned()
    }
}

pub(crate) fn say(response: &Value) -> String {
    format!("token {response}")
}

pub(crate) fn loaded_fake(replies: Vec<String>) -> FakeBackend {
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
