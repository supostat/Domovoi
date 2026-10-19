mod synthetic_tree;

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use domovoi_index::{Enrichment, Index};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet, SimulatorRuntimes, rule_table};
use domovoi_scanner::Node;
use synthetic_tree::{FILE_ALLOCATION, data_volume, directory, file, merge, path_tree};

const PROJECT: &str = "Users/igor/Projects/engine";
const BUILD_UNITS: usize = 1_000;
const OBJECTS_PER_UNIT: usize = 500;
const ATTEMPTS: usize = 3;
const INDEX_BUILDS_ALLOWED: u32 = 2;

struct NoSimulators;

impl SimulatorRuntimes for NoSimulators {
    fn device_runtime_missing(&self, _device: &Path) -> bool {
        false
    }
}

fn build_products() -> Node {
    directory(
        "target",
        (0..BUILD_UNITS)
            .map(|unit| {
                directory(
                    &format!("unit-{unit}"),
                    (0..OBJECTS_PER_UNIT)
                        .map(|object| file(&format!("object-{object}.o")))
                        .collect(),
                )
            })
            .collect(),
    )
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
        path_tree(PROJECT, build_products()),
    ]));
    let index_build = fastest(|| drop(Index::build(&report, root.clone(), &Enrichment::default())));
    let index = Index::build(&report, root, &Enrichment::default());
    let rules = RuleSet::new(
        rule_table(&ConfigOverrides::default()),
        CaseRule::Insensitive,
        Box::new(NoSimulators),
    );
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
