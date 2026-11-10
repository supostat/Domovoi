mod synthetic_tree;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domovoi_apfs_sys::chflags;
use domovoi_apfs_sys::sys::{EF_MAY_SHARE_BLOCKS, UF_COMPRESSED, UF_IMMUTABLE};
use domovoi_index::{
    ChangedPath, DeltaRequest, DirectoryListing, Enrichment, Index, IndexRoot, ListedEntry, Reclaimable, Timestamp,
};
use domovoi_rules::{
    CaseRule, ConfigOverrides, DenyReason, Denylist, Hypothesis, LevelTotals, Recommendations, RuleSet, rule_table,
};
use domovoi_scanner::{Node, NodeState, ScanOptions, ScanReport, scan};
use domovoi_testkit::{Fill, TempTree};
use synthetic_tree::{FILE_ALLOCATION, NoSimulators, directory, file};

const SHARED_CLONE_ID: u64 = 77;
const LONG_AGO_SECONDS: u64 = 1_600_000_000;

#[derive(Debug, PartialEq, Eq)]
struct CardView {
    items: BTreeMap<PathBuf, Reclaimable>,
    blocked: BTreeMap<PathBuf, DenyReason>,
    reclaimable: Reclaimable,
    owner_running: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct ProjectView {
    items: BTreeMap<PathBuf, Reclaimable>,
    blocked: BTreeMap<PathBuf, DenyReason>,
    reclaimable: Reclaimable,
    last_work: Timestamp,
}

#[derive(Debug, PartialEq, Eq)]
struct View {
    cards: BTreeMap<&'static str, CardView>,
    projects: BTreeMap<PathBuf, ProjectView>,
    totals: LevelTotals,
    joint: BTreeMap<Vec<PathBuf>, u64>,
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
                    };
                    (self.rules.rule(card.rule).id, view)
                })
                .collect(),
            projects: recommendations
                .projects
                .iter()
                .map(|project| {
                    let view = ProjectView {
                        items: project
                            .items
                            .iter()
                            .map(|item| (path(item.node), item.reclaimable))
                            .collect(),
                        blocked: project
                            .blocked
                            .iter()
                            .map(|blocked| (path(blocked.node), blocked.reason.clone()))
                            .collect(),
                        reclaimable: project.reclaimable,
                        last_work: project.last_work,
                    };
                    (path(project.root), view)
                })
                .collect(),
            totals: recommendations.totals,
            joint: recommendations
                .joint
                .iter()
                .map(|hold| {
                    let mut items: Vec<PathBuf> = hold.items.iter().map(|item| path(*item)).collect();
                    items.sort();
                    (items, hold.bytes)
                })
                .collect(),
            hypotheses: recommendations
                .hypotheses
                .iter()
                .map(|card| (path(card.node), card.reclaimable))
                .collect(),
        }
    }

    fn full_build(&self) -> Recommendations {
        Recommendations::build(
            &self.index,
            &self.rules,
            &self.denylist,
            &BTreeSet::new(),
            self.hypotheses.clone(),
        )
    }

    fn views(&self) -> (View, View) {
        (self.view(&self.recommendations), self.view(&self.full_build()))
    }

    fn assert_matches_a_full_build(&self) {
        let (delta, full) = self.views();
        assert_eq!(delta, full);
        assert_eq!(
            self.recommendations.joint,
            self.full_build().joint,
            "joint holds name the current nodes of their items"
        );
        assert!(
            self.recommendations
                .cards
                .is_sorted_by_key(|card| Reverse(card.reclaimable.bytes))
        );
        assert!(
            self.recommendations
                .projects
                .is_sorted_by_key(|project| Reverse(project.reclaimable.bytes))
        );
    }

    fn card(&self, rule: &str) -> Option<CardView> {
        self.view(&self.recommendations).cards.remove(rule)
    }

    fn project(&self, relative: &str) -> Option<ProjectView> {
        self.view(&self.recommendations).projects.remove(&self.path(relative))
    }

    fn joint(&self) -> BTreeMap<Vec<PathBuf>, u64> {
        self.view(&self.recommendations).joint
    }

    fn allocated(&self, relative: &str) -> u64 {
        let node = self
            .index
            .find(&self.path(relative))
            .unwrap_or_else(|| panic!("{relative} is indexed"));
        self.index.node(node).alloc_size
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
    age_files(tree.path());
    tree
}

fn age_files(folder: &Path) {
    let long_ago: SystemTime = UNIX_EPOCH + Duration::from_secs(LONG_AGO_SECONDS);
    for entry in fs::read_dir(folder).expect("the folder lists") {
        let path = entry.expect("the entry reads").path();
        if path.is_dir() {
            age_files(&path);
        } else {
            File::options()
                .write(true)
                .open(&path)
                .expect("the file opens")
                .set_modified(long_ago)
                .expect("the modification time is set");
        }
    }
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
fn a_cargo_manifest_next_to_target_turns_it_into_a_project() {
    let mut bench = Workbench::scanned(workshop(false));
    assert!(bench.project("engine").is_none());
    assert!(bench.project("site").is_some());
    fs::write(bench.path("readme"), b"workshop").expect("a file appears in the root");
    bench.apply(&[""]);
    bench.assert_matches_a_full_build();
    fs::write(bench.path("engine/Cargo.toml"), b"[package]\n").expect("a marker appears");
    bench.apply(&["engine"]);
    let project = bench
        .project("engine")
        .expect("the build folder belongs to its project");
    assert_eq!(project.items.keys().collect::<Vec<_>>(), [&bench.path("engine/target")]);
    assert!(bench.card("cargo.target").is_none());
    bench.assert_matches_a_full_build();
    fs::remove_file(bench.path("engine/Cargo.toml")).expect("the marker is removed");
    bench.apply(&["engine"]);
    assert!(bench.project("engine").is_none());
    bench.assert_matches_a_full_build();
}

#[test]
fn a_file_added_inside_target_grows_the_project_but_not_its_last_work() {
    let mut bench = Workbench::scanned(workshop(true));
    let before = bench
        .project("engine")
        .expect("the build folder belongs to its project");
    fs::write(bench.path("engine/target/debug/incremental.bin"), vec![9_u8; 50_000]).expect("a file is added");
    bench.apply(&["engine/target/debug"]);
    let after = bench.project("engine").expect("the project stays");
    assert!(
        after.reclaimable.bytes > before.reclaimable.bytes,
        "{after:?} after {before:?}"
    );
    assert_eq!(after.last_work, before.last_work);
    bench.assert_matches_a_full_build();
    fs::write(bench.path("engine/notes.md"), b"next to the build folder").expect("a file appears next to target");
    bench.apply(&["engine"]);
    let noted = bench.project("engine").expect("the project stays");
    assert_eq!(noted.reclaimable, after.reclaimable);
    assert!(noted.last_work > after.last_work, "{noted:?} after {after:?}");
    bench.assert_matches_a_full_build();
}

#[test]
fn editing_a_source_file_moves_the_last_work_of_its_project() {
    let mut bench = Workbench::scanned(workshop(true));
    let before = bench
        .project("engine")
        .expect("the build folder belongs to its project");
    fs::write(bench.path("engine/src/main.rs"), b"fn main() {}\n").expect("the source is edited");
    bench.apply(&["engine/src"]);
    let after = bench.project("engine").expect("the project stays");
    assert!(after.last_work > before.last_work, "{after:?} after {before:?}");
    assert_eq!(after.reclaimable, before.reclaimable);
    bench.assert_matches_a_full_build();
}

#[test]
fn removing_node_modules_drops_its_project() {
    let mut bench = Workbench::scanned(workshop(true));
    assert!(bench.project("site").is_some());
    fs::remove_dir_all(bench.path("site/node_modules")).expect("the dependencies are removed");
    bench.apply(&["site/node_modules"]);
    assert!(bench.project("site").is_none());
    assert!(bench.project("engine").is_some());
    bench.assert_matches_a_full_build();
}

#[test]
fn a_protected_flag_inside_an_item_blocks_it_in_its_project() {
    let mut bench = Workbench::scanned(workshop(true));
    let locked = bench.path("site/node_modules/left-pad.js");
    set_flags(&locked, UF_IMMUTABLE);
    bench.apply(&["site/node_modules"]);
    let project = bench.project("site");
    let views = bench.views();
    set_flags(&locked, 0);
    let project = project.expect("the dependencies stay in their project");
    assert!(project.items.is_empty());
    assert_eq!(
        project.blocked.values().collect::<Vec<_>>(),
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
        .project("engine")
        .expect("the build folder belongs to its project")
        .reclaimable;
    assert_eq!(shared.held_by_clones, FILE_ALLOCATION);
    bench.apply_entries("notes", vec![ListedEntry::Read(file("todo.txt"))]);
    let alone = bench
        .project("engine")
        .expect("the build folder stays in its project")
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

fn projects_sharing_a_hard_link_and_a_clone() -> TempTree {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    for project in ["alpha", "beta"] {
        let folder = root.directory(project).expect("directory is created");
        folder.file("package.json", 64, Fill::Byte(1)).expect("file is written");
        folder
            .directory("node_modules")
            .expect("directory is created")
            .directory("pkg")
            .expect("directory is created");
    }
    let alpha = tree.path().join("alpha/node_modules/pkg");
    let beta = tree.path().join("beta/node_modules/pkg");
    fs::write(alpha.join("linked.bin"), vec![3_u8; 40_000]).expect("file is written");
    fs::hard_link(alpha.join("linked.bin"), beta.join("linked.bin")).expect("the projects share a hard link");
    fs::write(alpha.join("cloned.bin"), vec![5_u8; 60_000]).expect("file is written");
    let status = Command::new("/bin/cp")
        .arg("-c")
        .arg(alpha.join("cloned.bin"))
        .arg(beta.join("cloned.bin"))
        .status()
        .expect("cp runs");
    assert!(status.success(), "cp -c failed");
    age_files(tree.path());
    tree
}

#[test]
fn a_delta_touching_one_of_two_projects_with_shared_files_keeps_their_joint_hold_as_a_full_build() {
    let mut bench = Workbench::scanned(projects_sharing_a_hard_link_and_a_clone());
    let together = vec![bench.path("alpha/node_modules"), bench.path("beta/node_modules")];
    let linked = bench.allocated("alpha/node_modules/pkg/linked.bin");
    let cloned = bench.allocated("alpha/node_modules/pkg/cloned.bin");
    assert_eq!(bench.joint(), BTreeMap::from([(together.clone(), linked + cloned)]));

    fs::write(bench.path("alpha/README.md"), b"alpha").expect("a file appears next to the dependencies");
    bench.apply(&["alpha"]);
    assert_eq!(bench.joint(), BTreeMap::from([(together.clone(), linked + cloned)]));
    bench.assert_matches_a_full_build();

    fs::write(bench.path("beta/node_modules/pkg/fresh.js"), b"export {}").expect("a file is added");
    bench.apply(&["beta/node_modules/pkg"]);
    assert_eq!(bench.joint(), BTreeMap::from([(together.clone(), linked + cloned)]));
    bench.assert_matches_a_full_build();

    fs::remove_file(bench.path("beta/node_modules/pkg/linked.bin")).expect("the shared link is removed");
    bench.apply(&["beta/node_modules/pkg"]);
    assert_eq!(bench.joint(), BTreeMap::from([(together, cloned)]));
    bench.assert_matches_a_full_build();

    fs::remove_file(bench.path("beta/node_modules/pkg/cloned.bin")).expect("the shared clone is removed");
    bench.apply(&["beta/node_modules/pkg"]);
    assert!(bench.joint().is_empty(), "{:?}", bench.joint());
    bench.assert_matches_a_full_build();
}

#[test]
fn removing_the_dependencies_of_one_of_two_projects_with_shared_files_drops_their_joint_hold() {
    let mut bench = Workbench::scanned(projects_sharing_a_hard_link_and_a_clone());
    assert_eq!(bench.joint().len(), 1, "{:?}", bench.joint());
    fs::remove_dir_all(bench.path("beta/node_modules")).expect("the dependencies are removed");
    bench.apply(&["beta"]);
    assert!(bench.project("beta").is_none());
    assert!(bench.joint().is_empty(), "{:?}", bench.joint());
    bench.assert_matches_a_full_build();
}

#[test]
fn a_third_project_linking_the_shared_file_joins_the_joint_hold() {
    let mut bench = Workbench::scanned(projects_sharing_a_hard_link_and_a_clone());
    let linked = bench.allocated("alpha/node_modules/pkg/linked.bin");
    let cloned = bench.allocated("alpha/node_modules/pkg/cloned.bin");
    let gamma = bench.path("gamma/node_modules/pkg");
    fs::create_dir_all(&gamma).expect("the dependencies folder is created");
    fs::write(bench.path("gamma/package.json"), b"{}").expect("the manifest is written");
    fs::hard_link(
        bench.path("alpha/node_modules/pkg/linked.bin"),
        gamma.join("linked.bin"),
    )
    .expect("the third project links the shared file");
    bench.apply(&[""]);
    let modules = |projects: &[&str]| -> Vec<PathBuf> {
        projects
            .iter()
            .map(|project| bench.path(&format!("{project}/node_modules")))
            .collect()
    };
    assert_eq!(
        bench.joint(),
        BTreeMap::from([
            (modules(&["alpha", "beta"]), cloned),
            (modules(&["alpha", "beta", "gamma"]), linked),
        ])
    );
    bench.assert_matches_a_full_build();
}
