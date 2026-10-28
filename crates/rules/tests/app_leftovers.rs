use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use domovoi_index::{Enrichment, Index, IndexRoot, SafetyLevel};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet, SimulatorRuntimes, rule_table};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{TempTree, app_container, application_bundle};

const LIBRARY: &str = "Users/tester/Library";

struct NoSimulators;

impl SimulatorRuntimes for NoSimulators {
    fn device_runtime_missing(&self, _device: &Path) -> bool {
        false
    }
}

fn library(root: &Path, relative: &str) -> PathBuf {
    root.join(LIBRARY).join(relative)
}

fn folder_with_payload(folder: &Path) {
    fs::create_dir_all(folder).expect("a folder is created");
    fs::write(folder.join("payload.bin"), vec![3_u8; 8_192]).expect("a payload is written");
}

fn volume_index(root: &Path) -> Index {
    let canonical = fs::canonicalize(root).expect("the root resolves");
    let report = scan(&canonical, &ScanOptions::default()).expect("the tree scans");
    let volume = IndexRoot::Volume {
        path: canonical.to_string_lossy().into_owned(),
    };
    Index::build(&report, volume, &Enrichment::default())
}

fn card_paths(index: &Index, rules: &RuleSet, recommendations: &Recommendations, id: &str) -> BTreeSet<String> {
    let root = index.path(index.root());
    recommendations
        .cards
        .iter()
        .filter(|card| rules.rule(card.rule).id == id)
        .flat_map(|card| &card.items)
        .map(|item| {
            index
                .path(item.node)
                .strip_prefix(&root)
                .expect("a card item lies on the volume")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn leftovers_of_removed_applications_fill_their_cards_by_exact_signals_only() {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.path();
    let applications = root.join("Applications");
    application_bundle(&applications, "Tiny", "com.domovoi.tiny", &["group.com.domovoi.shared"])
        .expect("the application is built");
    application_bundle(
        &applications,
        "Other",
        "com.domovoi.other",
        &["group.com.domovoi.shared"],
    )
    .expect("the application is built");
    app_container(&library(root, "Containers/com.example.renamed"), "com.domovoi.gone")
        .expect("the container is built");
    folder_with_payload(&library(root, "Containers/com.example.renamed/Data/Library/Caches"));
    folder_with_payload(&library(root, "Containers/com.apple.Notes"));
    folder_with_payload(&library(root, "Caches/com.domovoi.gone"));
    folder_with_payload(&library(root, "Caches/org.carthage.CarthageKit"));
    folder_with_payload(&library(root, "Group Containers/ZYXWV98765.com.domovoi.gone"));
    folder_with_payload(&library(root, "Group Containers/group.com.domovoi.shared"));
    folder_with_payload(&library(root, "Group Containers/group.com.apple.notes"));
    folder_with_payload(&library(root, "Application Support/Tiny"));
    folder_with_payload(&library(root, "Application Support/Gone"));
    let index = volume_index(root);
    let rules = RuleSet::new(
        rule_table(&ConfigOverrides::default()),
        CaseRule::Insensitive,
        Box::new(NoSimulators),
    );
    let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
    let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());

    let level = |id: &str| {
        rules
            .rules()
            .iter()
            .find(|rule| rule.id == id)
            .expect("the rule exists")
            .level
    };
    assert_eq!(level("apps.leftover-caches"), SafetyLevel::Safe);
    assert_eq!(level("apps.leftover-data"), SafetyLevel::CheckYourself);
    assert_eq!(
        card_paths(&index, &rules, &recommendations, "apps.leftover-caches"),
        BTreeSet::from([format!("{LIBRARY}/Caches/com.domovoi.gone")])
    );
    assert_eq!(
        card_paths(&index, &rules, &recommendations, "apps.leftover-data"),
        BTreeSet::from([
            format!("{LIBRARY}/Containers/com.example.renamed"),
            format!("{LIBRARY}/Group Containers/ZYXWV98765.com.domovoi.gone"),
        ]),
        "a removed container goes whole, caches inside included; shared, system and name-only folders stay"
    );
    assert_eq!(
        card_paths(&index, &rules, &recommendations, "cache.carthage"),
        BTreeSet::from([format!("{LIBRARY}/Caches/org.carthage.CarthageKit")]),
        "a vendor cache keeps its own rule"
    );
}
