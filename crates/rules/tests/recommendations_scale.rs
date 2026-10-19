mod synthetic_tree;

use std::collections::BTreeSet;
use std::fs;
use std::time::{Duration, Instant};

use domovoi_index::{ChangedPath, DeltaRequest, DirectoryListing, Enrichment, Index, IndexRoot, ListedEntry};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet, rule_table};
use domovoi_scanner::{Node, NodeState, ScanReport};
use domovoi_testkit::TempTree;
use synthetic_tree::{FILE_ALLOCATION, NoSimulators, data_volume, directory, file, merge, path_tree};

const PROJECT: &str = "Users/igor/Projects/engine";
const BUILD_UNITS: usize = 1_000;
const OBJECTS_PER_UNIT: usize = 500;
const WORKSHOP_UNITS: usize = 500;
const PACKAGES: usize = 500;
const FILES_PER_PACKAGE: usize = 500;
const LOOSE_NOTES: usize = 100;
const ATTEMPTS: usize = 3;
const INDEX_BUILDS_ALLOWED: u32 = 2;
const INDEX_BUILD_SHARE: u32 = 100;

struct Workshop {
    tree: TempTree,
    report: ScanReport,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    recommendations: Recommendations,
}

impl Workshop {
    fn new() -> Self {
        let tree = TempTree::new().expect("temp tree is created");
        for folder in ["engine", "engine/target", "engine/target/unit-0", "site", "notes"] {
            fs::create_dir(tree.path().join(folder)).expect("the folder exists on disk for its delta target");
        }
        let report = ScanReport {
            root: Node {
                name: tree.path().as_os_str().to_owned(),
                attributes: None,
                last_used: None,
                state: NodeState::Listed,
                children: vec![
                    directory("engine", vec![file("Cargo.toml"), build_products(WORKSHOP_UNITS)]),
                    directory("site", vec![file("package.json"), dependencies()]),
                    directory("notes", numbered("note", LOOSE_NOTES)),
                ],
            },
            files: 0,
            bytes: 0,
        };
        let index = Index::build(&report, Self::root(&tree), &Enrichment::default());
        let rules = rules();
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

    fn listing(&self, relative: &str, entries: Vec<ListedEntry>) -> (DeltaRequest, DirectoryListing) {
        let request = self.index.delta_request(&[ChangedPath {
            path: self.tree.path().join(relative),
            subtree: false,
        }]);
        let [target] = request.targets() else {
            panic!("one target is expected: {request:?}");
        };
        let listing = target.listing(entries, 0);
        (request, listing)
    }

    fn fastest_delta(&self, request: &DeltaRequest, listing: &DirectoryListing) -> (Duration, Duration) {
        let attempts: Vec<(Duration, Duration)> = (0..ATTEMPTS)
            .map(|_| {
                let mut index = self.index.clone();
                let mut recommendations = self.recommendations.clone();
                let listings = vec![listing.clone()];
                let started = Instant::now();
                let applied = index
                    .apply_delta(request, listings, &Enrichment::default())
                    .expect("the request is current");
                let index_delta = started.elapsed();
                let started = Instant::now();
                recommendations.apply_delta(&index, &self.rules, &self.denylist, &BTreeSet::new(), &applied);
                let cards_delta = started.elapsed();
                let full = Recommendations::build(&index, &self.rules, &self.denylist, &BTreeSet::new(), Vec::new());
                assert_eq!(recommendations.totals, full.totals);
                (index_delta, cards_delta)
            })
            .collect();
        (
            attempts
                .iter()
                .map(|attempt| attempt.0)
                .min()
                .expect("at least one attempt runs"),
            attempts
                .iter()
                .map(|attempt| attempt.1)
                .min()
                .expect("at least one attempt runs"),
        )
    }
}

fn rules() -> RuleSet {
    RuleSet::new(
        rule_table(&ConfigOverrides::default()),
        CaseRule::Insensitive,
        Box::new(NoSimulators),
    )
}

fn numbered(prefix: &str, count: usize) -> Vec<Node> {
    (0..count).map(|number| file(&format!("{prefix}-{number}"))).collect()
}

fn build_products(units: usize) -> Node {
    directory(
        "target",
        (0..units)
            .map(|unit| directory(&format!("unit-{unit}"), numbered("object", OBJECTS_PER_UNIT)))
            .collect(),
    )
}

fn dependencies() -> Node {
    directory(
        "node_modules",
        (0..PACKAGES)
            .map(|package| directory(&format!("package-{package}"), numbered("module", FILES_PER_PACKAGE)))
            .collect(),
    )
}

fn shallow(node: &Node) -> Node {
    Node {
        children: Vec::new(),
        ..node.clone()
    }
}

fn read_all(nodes: Vec<Node>) -> Vec<ListedEntry> {
    nodes.into_iter().map(ListedEntry::Read).collect()
}

fn fastest(mut run: impl FnMut()) -> Duration {
    (0..ATTEMPTS)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .min()
        .expect("at least one attempt runs")
}

#[test]
fn recommendations_over_half_a_million_files_sum_allocations_within_two_index_builds() {
    let (report, root) = data_volume(merge(vec![
        path_tree(PROJECT, file("Cargo.toml")),
        path_tree(PROJECT, build_products(BUILD_UNITS)),
    ]));
    let index_build = fastest(|| drop(Index::build(&report, root.clone(), &Enrichment::default())));
    let index = Index::build(&report, root, &Enrichment::default());
    let rules = rules();
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let mut recommendations = Recommendations::default();
    let recommendations_build = fastest(|| {
        recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());
    });

    let card = recommendations
        .cards
        .iter()
        .find(|card| rules.rule(card.rule).id == "cargo.target")
        .expect("the build folder of the project is a card");
    let objects = u64::try_from(BUILD_UNITS * OBJECTS_PER_UNIT).expect("the object count fits in u64");
    assert_eq!(card.reclaimable.bytes, objects * FILE_ALLOCATION);
    assert_eq!(card.reclaimable.held_by_snapshots, 0);
    assert!(
        recommendations_build <= index_build * INDEX_BUILDS_ALLOWED,
        "recommendations took {recommendations_build:?}, the index build {index_build:?}"
    );
}

#[test]
fn a_directory_delta_outside_the_cards_costs_at_most_a_hundredth_of_an_index_build() {
    let workshop = Workshop::new();
    let index_build = fastest(|| {
        drop(Index::build(
            &workshop.report,
            Workshop::root(&workshop.tree),
            &Enrichment::default(),
        ));
    });
    let engine = &workshop.report.root.children[0];
    let mut notes = numbered("note", LOOSE_NOTES);
    notes.push(file("fresh"));
    let deltas = [
        ("notes", workshop.listing("notes", read_all(notes))),
        (
            "engine",
            workshop.listing(
                "engine",
                vec![
                    ListedEntry::Read(file("Cargo.toml")),
                    ListedEntry::Read(file("README.md")),
                    ListedEntry::Kept(shallow(&engine.children[1])),
                ],
            ),
        ),
    ];
    for (name, (request, listing)) in deltas {
        let (index_delta, cards_delta) = workshop.fastest_delta(&request, &listing);
        assert!(
            (index_delta + cards_delta) * INDEX_BUILD_SHARE <= index_build,
            "the {name} delta took {index_delta:?} in the index and {cards_delta:?} on the cards, \
             the index build {index_build:?}"
        );
    }
}

#[test]
fn a_delta_inside_target_costs_at_most_one_recommendations_build() {
    let workshop = Workshop::new();
    let cards: BTreeSet<&str> = workshop
        .recommendations
        .cards
        .iter()
        .map(|card| workshop.rules.rule(card.rule).id)
        .collect();
    assert_eq!(cards, BTreeSet::from(["cargo.target", "dev.deps"]));
    let recommendations_build = fastest(|| {
        drop(Recommendations::build(
            &workshop.index,
            &workshop.rules,
            &workshop.denylist,
            &BTreeSet::new(),
            Vec::new(),
        ));
    });
    let mut objects = numbered("object", OBJECTS_PER_UNIT);
    objects.push(file("fresh.o"));
    let (request, listing) = workshop.listing("engine/target/unit-0", read_all(objects));
    let (_, cards_delta) = workshop.fastest_delta(&request, &listing);
    assert!(
        cards_delta <= recommendations_build,
        "the cards delta took {cards_delta:?}, a recommendations build {recommendations_build:?}"
    );
}
