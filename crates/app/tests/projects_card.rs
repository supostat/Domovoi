use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use domovoi_app::{CardModel, Provenance, RecommendationsModel, TrayModel, strings};
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet, Tray};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const NINETY_DAYS: Duration = Duration::from_secs(90 * 24 * 60 * 60);

struct Workspace {
    tree: TempTree,
    index: Index,
    rules: RuleSet,
    recommendations: Recommendations,
}

impl Workspace {
    fn new() -> Self {
        let tree = TempTree::new().expect("a temp tree is created");
        let root = tree.root().expect("the root opens");
        for (project, size) in [("archive", 300_000), ("engine", 90_000)] {
            let folder = root.directory(project).expect("a project folder is created");
            folder
                .file("Cargo.toml", 64, Fill::Byte(1))
                .expect("a manifest is written");
            folder
                .directory("target")
                .expect("a build folder is created")
                .file("build.bin", size, Fill::Noise(2))
                .expect("build output is written");
        }
        let mono = root.directory("mono").expect("a repository is created");
        mono.directory(".git")
            .expect("a repository folder is created")
            .file("HEAD", 32, Fill::Byte(3))
            .expect("a repository file is written");
        let packages = mono.directory("packages").expect("a packages folder is created");
        for package in ["a", "b"] {
            let folder = packages.directory(package).expect("a package is created");
            folder
                .file("package.json", 64, Fill::Byte(4))
                .expect("a manifest is written");
            folder
                .directory("node_modules")
                .expect("dependencies are created")
                .file("index.js", 50_000, Fill::Noise(5))
                .expect("a dependency is written");
        }
        mono.file("next.config.js", 64, Fill::Byte(6))
            .expect("a config is written");
        mono.directory(".next")
            .expect("a build cache is created")
            .file("cache.bin", 20_000, Fill::Noise(7))
            .expect("a cache file is written");
        let long_ago = SystemTime::now() - NINETY_DAYS;
        age_files(&tree.path().join("archive"), long_ago);
        age_files(&tree.path().join("mono"), long_ago);
        let report = scan(tree.path(), &ScanOptions::default()).expect("the tree scans");
        let index = Index::build(
            &report,
            IndexRoot::for_path(tree.path()).expect("the root kind is known"),
            &Enrichment::default(),
        );
        let rules = RuleSet::for_volume(tree.path(), &ConfigOverrides::default()).expect("the rules are prepared");
        let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
        let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());
        Self {
            tree,
            index,
            rules,
            recommendations,
        }
    }

    fn model(&self) -> RecommendationsModel {
        RecommendationsModel::build(&self.index, &self.rules, &self.recommendations, SystemTime::now())
    }
}

fn age_files(folder: &Path, when: SystemTime) {
    for entry in fs::read_dir(folder).expect("the folder lists") {
        let path = entry.expect("the entry reads").path();
        if path.is_dir() {
            age_files(&path, when);
        } else {
            File::options()
                .write(true)
                .open(&path)
                .expect("the file opens")
                .set_modified(when)
                .expect("the modification time is set");
        }
    }
}

fn projects(model: &RecommendationsModel) -> &CardModel {
    model
        .cards
        .iter()
        .find(|card| card.provenance == Provenance::Projects)
        .expect("the workspace has a projects card")
}

fn projects_mut(model: &mut RecommendationsModel) -> &mut CardModel {
    model
        .cards
        .iter_mut()
        .find(|card| card.provenance == Provenance::Projects)
        .expect("the workspace has a projects card")
}

fn names(card: &CardModel) -> Vec<&str> {
    card.rows.iter().map(|row| row.name.as_str()).collect()
}

#[test]
fn one_card_lists_every_project_with_stale_projects_first_and_none_checked() {
    let workspace = Workspace::new();
    let model = workspace.model();
    let card = projects(&model);
    assert_eq!(card.title, strings::PROJECTS_TITLE);
    assert_eq!(names(card), ["archive", "mono", "engine"]);
    let stale: Vec<(bool, bool)> = card.rows.iter().map(|row| (row.stale, row.selected)).collect();
    assert_eq!(stale, [(true, false), (true, false), (false, false)]);
    assert!(!card.some_row_selected() && !card.selected);
    assert_eq!(card.selected_amount(), 0);
    assert_eq!(card.amount, card.rows.iter().map(|row| row.amount).sum::<u64>());
    assert_eq!(card.path_line(), "3\u{a0}проекта");
    assert!(
        model
            .cards
            .iter()
            .all(|other| other.provenance == Provenance::Projects || other.rows.is_empty()),
        "only the projects card has rows"
    );
}

#[test]
fn a_row_names_its_root_artifacts_and_their_repeats() {
    let workspace = Workspace::new();
    let model = workspace.model();
    let mono = &projects(&model).rows[1];
    assert_eq!(mono.name, "mono");
    assert!(mono.path.ends_with("/mono"), "{}", mono.path);
    assert_eq!(mono.artifacts, ["node_modules ×2", ".next"]);
    let fresh = &projects(&model).rows[2];
    assert_eq!(fresh.age, "сегодня");
}

#[test]
fn toggling_rows_groups_and_the_card_moves_the_selection() {
    let workspace = Workspace::new();
    let mut model = workspace.model();
    let card = projects_mut(&mut model);
    card.toggle_group(true);
    assert_eq!(
        card.rows.iter().map(|row| row.selected).collect::<Vec<_>>(),
        [true, true, false]
    );
    assert!(card.some_row_selected() && !card.selected);
    card.toggle_row(2);
    assert!(card.every_row_selected() && card.selected);
    card.toggle_group(true);
    assert_eq!(
        card.rows.iter().map(|row| row.selected).collect::<Vec<_>>(),
        [false, false, true]
    );
    card.toggle_rows();
    assert!(card.every_row_selected() && card.selected);
    card.toggle_rows();
    assert!(!card.some_row_selected() && !card.selected);
    assert_eq!(card.selected_amount(), 0);
}

#[test]
fn the_selection_of_rows_survives_a_rebuild() {
    let workspace = Workspace::new();
    let mut previous = workspace.model();
    let card = projects_mut(&mut previous);
    card.toggle_row(0);
    card.toggle_row(2);
    let mut rebuilt = workspace.model();
    rebuilt.keep_selection(&previous);
    assert_eq!(
        projects(&rebuilt)
            .rows
            .iter()
            .map(|row| (row.name.as_str(), row.selected))
            .collect::<Vec<_>>(),
        [("archive", true), ("mono", false), ("engine", true)]
    );
}

#[test]
fn selected_rows_go_to_the_tray_as_one_row_per_project() {
    let workspace = Workspace::new();
    let mut model = workspace.model();
    let card = projects_mut(&mut model);
    card.toggle_group(true);
    let targets = card.selected_targets();
    assert_eq!(
        targets.len(),
        4,
        "archive/target and the three artifacts of mono: {targets:?}"
    );
    let mut tray = Tray::default();
    for target in &targets {
        let project = target.project.as_ref().expect("a project artifact names its project");
        let rule = target.rule_id.as_deref().expect("a project artifact names its rule");
        assert!(project.starts_with(workspace.tree.path()), "{project:?}");
        tray.add_project_artifact(&workspace.index, target.node, rule, project);
    }
    let sheet = TrayModel::estimated(&workspace.index, &tray);
    assert_eq!(
        sheet.rows.iter().map(|row| row.title.as_str()).collect::<Vec<_>>(),
        ["archive", "mono"]
    );
    assert_eq!(sheet.rows[1].details.len(), 3, "{:?}", sheet.rows[1].details);
    assert_eq!(sheet.row_nodes[1].len(), 3);
}
