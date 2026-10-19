mod synthetic_tree;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::chflags;
use domovoi_apfs_sys::sys::{EF_MAY_SHARE_BLOCKS, UF_COMPRESSED, UF_IMMUTABLE};
use domovoi_index::{
    ChangedPath, DeltaRequest, DirectoryListing, Enrichment, Index, IndexRoot, ListedEntry, Reclaimable,
};
use domovoi_rules::{
    CaseRule, ConfigOverrides, DenyReason, Denylist, Hypothesis, LevelTotals, Recommendations, RuleSet, rule_table,
};
use domovoi_scanner::{Node, NodeState, ScanOptions, ScanReport, scan};
use domovoi_testkit::{Fill, TempTree};
use synthetic_tree::{FILE_ALLOCATION, NoSimulators, directory, file};

const SHARED_CLONE_ID: u64 = 77;

#[derive(Debug, PartialEq, Eq)]
struct CardView {
    items: BTreeMap<PathBuf, Reclaimable>,
    blocked: BTreeMap<PathBuf, DenyReason>,
    reclaimable: Reclaimable,
    owner_running: bool,
    preselected: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct View {
    cards: BTreeMap<&'static str, CardView>,
    totals: LevelTotals,
    hypotheses: BTreeMap<PathBuf, Reclaimable>,
}

struct Workbench {
    tree: TempTree,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    hypotheses: Vec<Hypothesis>,
    recommendations: Recommendations,
}

impl Workbench {
    fn scanned(tree: TempTree) -> Self {
        let report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
        let rules = RuleSet::for_volume(tree.path(), &ConfigOverrides::default()).expect("rules are prepared");
        Self::of(tree, &report, rules)
    }

    fn synthetic(tree: TempTree, children: Vec<Node>) -> Self {
        let report = ScanReport {
            root: Node {
                name: tree.path().as_os_str().to_owned(),
                attributes: None,
                last_used: None,
                state: NodeState::Listed,
                children,
            },
            files: 0,
            bytes: 0,
        };
        let rules = RuleSet::new(
            rule_table(&ConfigOverrides::default()),
            CaseRule::Insensitive,
            Box::new(NoSimulators),
        );
        Self::of(tree, &report, rules)
    }

    fn of(tree: TempTree, report: &ScanReport, rules: RuleSet) -> Self {
        let index = Index::build(
            report,
            IndexRoot::for_path(tree.path()).expect("root kind"),
            &Enrichment::default(),
        );
        let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
        let hypotheses = vec![Hypothesis {
            path: tree.path().join("site"),
            explanation: "сайт целиком".to_owned(),
        }];
        let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), hypotheses.clone());
        Self {
            tree,
            index,
            rules,
            denylist,
            hypotheses,
            recommendations,
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        if relative.is_empty() {
            self.tree.path().to_path_buf()
        } else {
            self.tree.path().join(relative)
        }
    }

    fn request(&self, changed: &[&str]) -> DeltaRequest {
        let changed: Vec<ChangedPath> = changed
            .iter()
            .map(|relative| ChangedPath {
                path: self.path(relative),
                subtree: false,
            })
            .collect();
        self.index.delta_request(&changed)
    }

    fn apply(&mut self, changed: &[&str]) {
        let request = self.request(changed);
        let listings = request
            .targets()
            .iter()
            .map(|target| target.read(&ScanOptions::default()).expect("the target reads"))
            .collect();
        self.apply_listings(&request, listings);
    }

    fn apply_entries(&mut self, changed: &str, entries: Vec<ListedEntry>) {
        let request = self.request(&[changed]);
        let [target] = request.targets() else {
            panic!("one target is expected: {request:?}");
        };
        let listing = target.listing(entries, 0);
        self.apply_listings(&request, vec![listing]);
    }

    fn apply_listings(&mut self, request: &DeltaRequest, listings: Vec<DirectoryListing>) {
        let applied = self
            .index
            .apply_delta(request, listings, &Enrichment::default())
            .expect("the request is current");
        self.recommendations
            .apply_delta(&self.index, &self.rules, &self.denylist, &BTreeSet::new(), &applied);
    }

    fn view(&self, recommendations: &Recommendations) -> View {
        let path = |node| self.index.path(node);
        View {
            cards: recommendations
                .cards
                .iter()
                .map(|card| {
                    let view = CardView {
                        items: card
                            .items
                            .iter()
                            .map(|item| (path(item.node), item.reclaimable))
                            .collect(),
                        blocked: card
                            .blocked
                            .iter()
                            .map(|(node, reason)| (path(*node), reason.clone()))
                            .collect(),
                        reclaimable: card.reclaimable,
                        owner_running: card.owner_running,
                        preselected: card.preselected,
                    };
                    (self.rules.rule(card.rule).id, view)
                })
                .collect(),
            totals: recommendations.totals,
            hypotheses: recommendations
                .hypotheses
                .iter()
                .map(|card| (path(card.node), card.reclaimable))
                .collect(),
        }
    }

    fn views(&self) -> (View, View) {
        let full = Recommendations::build(
            &self.index,
            &self.rules,
            &self.denylist,
            &BTreeSet::new(),
            self.hypotheses.clone(),
        );
        (self.view(&self.recommendations), self.view(&full))
    }

    fn assert_matches_a_full_build(&self) {
        let (delta, full) = self.views();
        assert_eq!(delta, full);
        assert!(
            self.recommendations
                .cards
                .is_sorted_by_key(|card| Reverse(card.reclaimable.bytes))
        );
    }

    fn card(&self, rule: &str) -> Option<CardView> {
        self.view(&self.recommendations).cards.remove(rule)
    }
}

fn workshop(with_cargo_manifest: bool) -> TempTree {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let engine = root.directory("engine").expect("directory is created");
    if with_cargo_manifest {
        engine.file("Cargo.toml", 64, Fill::Byte(1)).expect("file is written");
    }
    engine
        .directory("src")
        .expect("directory is created")
        .file("main.rs", 4_096, Fill::Byte(2))
        .expect("file is written");
    engine
        .directory("target")
        .expect("directory is created")
        .directory("debug")
        .expect("directory is created")
        .file("engine", 90_000, Fill::Noise(3))
        .expect("file is written");
    let site = root.directory("site").expect("directory is created");
    site.file("package.json", 64, Fill::Byte(4)).expect("file is written");
    let modules = site.directory("node_modules").expect("directory is created");
    modules
        .file("left-pad.js", 40_000, Fill::Noise(5))
        .expect("file is written");
    modules
        .directory("lodash")
        .expect("directory is created")
        .file("index.js", 30_000, Fill::Noise(6))
        .expect("file is written");
    root.directory("notes")
        .expect("directory is created")
        .file("todo.txt", 4_096, Fill::Byte(7))
        .expect("file is written");
    tree
}

fn set_flags(path: &Path, flags: u32) {
    let file = File::open(path).expect("file opens");
    chflags(file.as_fd(), flags).expect("flags are set");
}

fn compressed_clone(name: &str) -> Node {
    let mut node = file(name);
    let attributes = node.attributes.as_mut().expect("a synthetic file has attributes");
    attributes.flags = UF_COMPRESSED;
    attributes.ext_flags = Some(u64::from(EF_MAY_SHARE_BLOCKS));
    attributes.clone_id = Some(SHARED_CLONE_ID);
    node
}

#[test]
fn a_cargo_manifest_next_to_target_turns_it_into_a_card() {
    let mut bench = Workbench::scanned(workshop(false));
    assert!(bench.card("cargo.target").is_none());
    assert!(bench.card("dev.deps").is_some());
    fs::write(bench.path("readme"), b"workshop").expect("a file appears in the root");
    bench.apply(&[""]);
    bench.assert_matches_a_full_build();
    fs::write(bench.path("engine/Cargo.toml"), b"[package]\n").expect("a marker appears");
    bench.apply(&["engine"]);
    let card = bench.card("cargo.target").expect("the build folder is a card");
    assert_eq!(card.items.keys().collect::<Vec<_>>(), [&bench.path("engine/target")]);
    bench.assert_matches_a_full_build();
    fs::remove_file(bench.path("engine/Cargo.toml")).expect("the marker is removed");
    bench.apply(&["engine"]);
    assert!(bench.card("cargo.target").is_none());
    bench.assert_matches_a_full_build();
}

#[test]
fn a_file_added_inside_target_grows_its_estimate() {
    let mut bench = Workbench::scanned(workshop(true));
    let before = bench
        .card("cargo.target")
        .expect("the build folder is a card")
        .reclaimable;
    fs::write(bench.path("engine/target/debug/incremental.bin"), vec![9_u8; 50_000]).expect("a file is added");
    bench.apply(&["engine/target/debug"]);
    let after = bench
        .card("cargo.target")
        .expect("the build folder stays a card")
        .reclaimable;
    assert!(after.bytes > before.bytes, "{after:?} after {before:?}");
    bench.assert_matches_a_full_build();
    fs::write(bench.path("engine/notes.md"), b"next to the build folder").expect("a file appears next to target");
    bench.apply(&["engine"]);
    assert_eq!(
        bench
            .card("cargo.target")
            .expect("the build folder stays a card")
            .reclaimable,
        after
    );
    bench.assert_matches_a_full_build();
}

#[test]
fn removing_node_modules_drops_the_dependencies_card() {
    let mut bench = Workbench::scanned(workshop(true));
    assert!(bench.card("dev.deps").is_some());
    fs::remove_dir_all(bench.path("site/node_modules")).expect("the dependencies are removed");
    bench.apply(&["site/node_modules"]);
    assert!(bench.card("dev.deps").is_none());
    assert!(bench.card("cargo.target").is_some());
    bench.assert_matches_a_full_build();
}

#[test]
fn a_protected_flag_inside_an_item_blocks_it() {
    let mut bench = Workbench::scanned(workshop(true));
    let locked = bench.path("site/node_modules/left-pad.js");
    set_flags(&locked, UF_IMMUTABLE);
    bench.apply(&["site/node_modules"]);
    let card = bench.card("dev.deps");
    let views = bench.views();
    set_flags(&locked, 0);
    let card = card.expect("the dependencies stay on their card");
    assert!(card.items.is_empty());
    assert_eq!(
        card.blocked.values().collect::<Vec<_>>(),
        [&DenyReason::ProtectedFlag("UF_IMMUTABLE")]
    );
    assert_eq!(views.0, views.1);
}

#[test]
fn a_clone_leaving_its_group_outside_an_item_frees_the_shared_blocks_of_the_item() {
    let tree = TempTree::new().expect("temp tree is created");
    for folder in ["engine", "engine/target", "notes"] {
        fs::create_dir(tree.path().join(folder)).expect("the folder exists on disk for its delta target");
    }
    let mut bench = Workbench::synthetic(
        tree,
        vec![
            directory(
                "engine",
                vec![
                    file("Cargo.toml"),
                    directory("target", vec![compressed_clone("symbols.dat")]),
                ],
            ),
            directory("notes", vec![compressed_clone("symbols.dat"), file("todo.txt")]),
        ],
    );
    let shared = bench
        .card("cargo.target")
        .expect("the build folder is a card")
        .reclaimable;
    assert_eq!(shared.held_by_clones, FILE_ALLOCATION);
    bench.apply_entries("notes", vec![ListedEntry::Read(file("todo.txt"))]);
    let alone = bench
        .card("cargo.target")
        .expect("the build folder stays a card")
        .reclaimable;
    assert_eq!(
        alone,
        Reclaimable {
            bytes: FILE_ALLOCATION,
            ..Reclaimable::default()
        }
    );
    bench.assert_matches_a_full_build();
}
