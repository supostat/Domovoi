use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use domovoi_apfs_sys::{EntryAttributes, ObjectKind};
use domovoi_app::HEAVY_DELTA_NODES;
use domovoi_index::{ChangedPath, Enrichment, Index, IndexRoot, ListedEntry};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet, SimulatorRuntimes, rule_table};
use domovoi_scanner::{Node, NodeState, ScanReport};
use domovoi_testkit::TempTree;

const UNITS: usize = 1_000;
const OBJECTS_PER_UNIT: usize = 500;
const OBJECT_SIZE: u64 = 4_096;
const ATTEMPTS: usize = 3;
const CREATED: u64 = 1_700_000_000;
const THRESHOLD_SHARE_OF_BUILD: f64 = 0.48;

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

fn numbered(prefix: &str, count: usize) -> Vec<Node> {
    (0..count).map(|number| file(&format!("{prefix}-{number}"))).collect()
}

fn engine() -> Node {
    directory(
        "engine",
        vec![
            file("Cargo.toml"),
            directory(
                "target",
                (0..UNITS)
                    .map(|unit| directory(&format!("unit-{unit}"), numbered("object", OBJECTS_PER_UNIT)))
                    .collect(),
            ),
        ],
    )
}

fn fastest(mut attempt: impl FnMut() -> Duration) -> Duration {
    (0..ATTEMPTS)
        .map(|_| attempt())
        .min()
        .expect("at least one attempt runs")
}

struct Volume {
    tree: TempTree,
    report: ScanReport,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    recommendations: Recommendations,
}

impl Volume {
    fn new() -> Self {
        let tree = TempTree::new().expect("temp tree is created");
        fs::create_dir(tree.path().join("engine")).expect("the project exists on disk");
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
        let index = Index::build(&report, Self::root(&tree), &Enrichment::default());
        let rules = RuleSet::new(
            rule_table(&ConfigOverrides::default()),
            CaseRule::Insensitive,
            Box::new(NoSimulators),
        );
        let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
        let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());
        Self {
            tree,
            report,
            index,
            rules,
            denylist,
            recommendations,
        }
    }

    fn root(tree: &TempTree) -> IndexRoot {
        IndexRoot::for_path(tree.path()).expect("root kind")
    }

    fn index_build(&self) -> Duration {
        fastest(|| {
            let started = Instant::now();
            drop(Index::build(
                &self.report,
                Self::root(&self.tree),
                &Enrichment::default(),
            ));
            started.elapsed()
        })
    }

    fn apply_new_folder(&self, files: usize) -> Duration {
        let request = self.index.delta_request(&[ChangedPath {
            path: self.tree.path().to_path_buf(),
            subtree: false,
        }]);
        let [target] = request.targets() else {
            panic!("the root is the only target: {request:?}");
        };
        let mut kept = self.report.root.children[0].clone();
        kept.children.clear();
        let listing = target.listing(
            vec![
                ListedEntry::Kept(kept),
                ListedEntry::Read(directory("fresh", numbered("file", files - 1))),
            ],
            1,
        );
        assert_eq!(listing.nodes(), files + 1);
        fastest(|| {
            let mut index = self.index.clone();
            let mut recommendations = self.recommendations.clone();
            let listings = vec![listing.clone()];
            let started = Instant::now();
            let applied = index
                .apply_delta(&request, listings, &Enrichment::default())
                .expect("the request is current");
            recommendations.apply_delta(&index, &self.rules, &self.denylist, &BTreeSet::new(), &applied);
            started.elapsed()
        })
    }
}

#[test]
fn a_batch_at_the_heavy_threshold_applies_within_its_measured_share_of_an_index_build() {
    let volume = Volume::new();
    let index_build = volume.index_build();
    let threshold = volume.apply_new_folder(HEAVY_DELTA_NODES);
    assert!(
        threshold.as_secs_f64() <= index_build.as_secs_f64() * THRESHOLD_SHARE_OF_BUILD,
        "a batch of {HEAVY_DELTA_NODES} nodes took {threshold:?}, the index build {index_build:?}"
    );
}
