use std::collections::BTreeSet;
use std::path::PathBuf;

use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

#[test]
fn a_real_index_yields_projects_with_level_totals() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let site = root.directory("site").expect("directory is created");
    site.file("package.json", 64, Fill::Byte(1)).expect("file is written");
    site.directory("node_modules")
        .expect("directory is created")
        .file("left-pad.js", 40_000, Fill::Noise(1))
        .expect("file is written");
    let engine = root.directory("engine").expect("directory is created");
    engine.file("Cargo.toml", 64, Fill::Byte(2)).expect("file is written");
    engine
        .directory("target")
        .expect("directory is created")
        .file("engine", 90_000, Fill::Noise(2))
        .expect("file is written");
    root.directory("notes")
        .expect("directory is created")
        .file("todo.txt", 64, Fill::Byte(3))
        .expect("file is written");

    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    let index = Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        &Enrichment::default(),
    );
    let rules = RuleSet::for_volume(tree.path(), &ConfigOverrides::default()).expect("rules are prepared");
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());

    assert!(recommendations.cards.is_empty(), "{:?}", recommendations.cards);
    let roots: BTreeSet<PathBuf> = recommendations
        .projects
        .iter()
        .map(|project| index.path(project.root))
        .collect();
    assert_eq!(
        roots,
        BTreeSet::from([tree.path().join("engine"), tree.path().join("site")])
    );
    let mut artifacts: BTreeSet<&str> = BTreeSet::new();
    for project in &recommendations.projects {
        assert!(project.reclaimable.bytes > 0);
        assert!(project.blocked.is_empty());
        assert!(project.last_work.is_known());
        artifacts.extend(project.items.iter().map(|item| rules.rule(item.rule).id));
    }
    assert_eq!(artifacts, BTreeSet::from(["cargo.target", "dev.deps"]));
    let project_total: u64 = recommendations
        .projects
        .iter()
        .map(|project| project.reclaimable.bytes)
        .sum();
    assert_eq!(recommendations.totals.safe, project_total);
    assert_eq!(recommendations.totals.total(), project_total);
}
