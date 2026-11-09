mod synthetic_tree;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use domovoi_apfs_sys::sys::UF_IMMUTABLE;
use domovoi_index::{Index, SafetyLevel, Timestamp};
use domovoi_rules::{CaseRule, ConfigOverrides, DenyReason, Denylist, Recommendations, RuleSet, rule_table};
use domovoi_scanner::Node;
use synthetic_tree::{NoSimulators, data_volume_index, directory, file, merge, path_tree};

const EPOCH_SECONDS: u64 = 1_700_000_000;
const SHARED_TARGET: &str = "/Users/igor/build/shared-target";

#[derive(Debug, PartialEq, Eq)]
struct ProjectView {
    items: BTreeSet<PathBuf>,
    blocked: BTreeMap<PathBuf, DenyReason>,
    last_work: Timestamp,
}

fn moment(offset: u64) -> Timestamp {
    Timestamp::from_system_time(UNIX_EPOCH + Duration::from_secs(EPOCH_SECONDS + offset))
}

fn modified_at(mut node: Node, offset: u64) -> Node {
    node.attributes
        .as_mut()
        .expect("a synthetic node has attributes")
        .modified = UNIX_EPOCH + Duration::from_secs(EPOCH_SECONDS + offset);
    node
}

fn immutable(mut node: Node) -> Node {
    node.attributes.as_mut().expect("a synthetic node has attributes").flags = UF_IMMUTABLE;
    node
}

fn workspace() -> Index {
    data_volume_index(merge(vec![
        path_tree("/Users/igor/.git", file("config")),
        path_tree(
            "/Users/igor/Projects",
            directory(
                "demo",
                vec![
                    file("Cargo.toml"),
                    directory("src", vec![modified_at(file("main.rs"), 100)]),
                    directory("target", vec![directory("debug", vec![modified_at(file("demo"), 300)])]),
                ],
            ),
        ),
        path_tree(
            "/Users/igor/Projects",
            directory(
                "site",
                vec![
                    directory(".git", vec![modified_at(file("FETCH_HEAD"), 500)]),
                    file("package.json"),
                    file("next.config.js"),
                    file("turbo.json"),
                    directory("app", vec![modified_at(file("page.tsx"), 200)]),
                    directory(
                        "node_modules",
                        vec![directory("react", vec![modified_at(file("index.js"), 400)])],
                    ),
                    directory(".next", vec![directory("cache", vec![modified_at(file("chunk"), 450)])]),
                    directory(".turbo", vec![modified_at(file("log"), 460)]),
                ],
            ),
        ),
        path_tree(
            "/Users/igor/Projects",
            directory(
                "mono",
                vec![
                    directory(".git", vec![file("HEAD")]),
                    file("package.json"),
                    directory(
                        "packages",
                        vec![
                            directory(
                                "a",
                                vec![
                                    file("package.json"),
                                    modified_at(file("index.js"), 250),
                                    directory("node_modules", vec![modified_at(file("left-pad.js"), 600)]),
                                ],
                            ),
                            directory(
                                "b",
                                vec![file("package.json"), directory("node_modules", vec![file("lodash.js")])],
                            ),
                        ],
                    ),
                ],
            ),
        ),
        path_tree(
            "/Users/igor/Projects",
            directory(
                "locked",
                vec![
                    file("package.json"),
                    directory("node_modules", vec![immutable(file("frozen.js"))]),
                ],
            ),
        ),
        path_tree(SHARED_TARGET, file("out")),
    ]))
}

fn recommendations(index: &Index) -> (RuleSet, Recommendations) {
    let rules = RuleSet::new(
        rule_table(&ConfigOverrides {
            cargo_target: Some(PathBuf::from(SHARED_TARGET)),
            ..ConfigOverrides::default()
        }),
        CaseRule::Insensitive,
        Box::new(NoSimulators),
    );
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let recommendations = Recommendations::build(index, &rules, &denylist, &BTreeSet::new(), Vec::new());
    (rules, recommendations)
}

fn projects(index: &Index, recommendations: &Recommendations) -> BTreeMap<PathBuf, ProjectView> {
    recommendations
        .projects
        .iter()
        .map(|project| {
            let view = ProjectView {
                items: project.items.iter().map(|item| index.path(item.node)).collect(),
                blocked: project
                    .blocked
                    .iter()
                    .map(|blocked| (index.path(blocked.node), blocked.reason.clone()))
                    .collect(),
                last_work: project.last_work,
            };
            (index.path(project.root), view)
        })
        .collect()
}

fn project(index: &Index, recommendations: &Recommendations, root: &str) -> ProjectView {
    projects(index, recommendations)
        .remove(&PathBuf::from(root))
        .unwrap_or_else(|| panic!("{root} is a project"))
}

fn paths(paths: &[&str]) -> BTreeSet<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

#[test]
fn a_folder_without_a_repository_is_the_root_of_its_own_artifacts() {
    let index = workspace();
    let (_, recommendations) = recommendations(&index);
    let demo = project(&index, &recommendations, "/Users/igor/Projects/demo");
    assert_eq!(demo.items, paths(&["/Users/igor/Projects/demo/target"]));
}

#[test]
fn a_repository_in_the_home_folder_never_becomes_a_project_root() {
    let index = workspace();
    let (_, recommendations) = recommendations(&index);
    let roots: BTreeSet<PathBuf> = projects(&index, &recommendations).into_keys().collect();
    assert!(!roots.contains(&PathBuf::from("/Users/igor")), "{roots:?}");
    assert!(roots.contains(&PathBuf::from("/Users/igor/Projects/demo")), "{roots:?}");
}

#[test]
fn every_artifact_of_one_root_lands_in_one_project() {
    let index = workspace();
    let (_, recommendations) = recommendations(&index);
    let site = project(&index, &recommendations, "/Users/igor/Projects/site");
    assert_eq!(
        site.items,
        paths(&[
            "/Users/igor/Projects/site/node_modules",
            "/Users/igor/Projects/site/.next",
            "/Users/igor/Projects/site/.turbo",
        ])
    );
}

#[test]
fn packages_of_a_repository_belong_to_the_repository() {
    let index = workspace();
    let (_, recommendations) = recommendations(&index);
    let mono = project(&index, &recommendations, "/Users/igor/Projects/mono");
    assert_eq!(
        mono.items,
        paths(&[
            "/Users/igor/Projects/mono/packages/a/node_modules",
            "/Users/igor/Projects/mono/packages/b/node_modules",
        ])
    );
    let roots: BTreeSet<PathBuf> = projects(&index, &recommendations).into_keys().collect();
    assert!(
        !roots.contains(&PathBuf::from("/Users/igor/Projects/mono/packages/a")),
        "{roots:?}"
    );
}

#[test]
fn the_shared_build_folder_stays_on_the_card_of_its_rule() {
    let index = workspace();
    let (rules, recommendations) = recommendations(&index);
    let card = recommendations
        .cards
        .iter()
        .find(|card| rules.rule(card.rule).id == "cargo.target")
        .expect("the shared build folder has a card");
    let items: BTreeSet<PathBuf> = card.items.iter().map(|item| index.path(item.node)).collect();
    assert_eq!(items, paths(&[SHARED_TARGET]));
    let in_projects = recommendations
        .projects
        .iter()
        .flat_map(|project| &project.items)
        .any(|item| index.path(item.node) == Path::new(SHARED_TARGET));
    assert!(!in_projects);
}

#[test]
fn the_last_work_ignores_build_folders_and_the_repository() {
    let index = workspace();
    let (_, recommendations) = recommendations(&index);
    assert_eq!(
        project(&index, &recommendations, "/Users/igor/Projects/demo").last_work,
        moment(100)
    );
    assert_eq!(
        project(&index, &recommendations, "/Users/igor/Projects/site").last_work,
        moment(200)
    );
    assert_eq!(
        project(&index, &recommendations, "/Users/igor/Projects/mono").last_work,
        moment(250)
    );
}

#[test]
fn a_protected_artifact_is_blocked_in_its_project() {
    let index = workspace();
    let (_, recommendations) = recommendations(&index);
    let locked = project(&index, &recommendations, "/Users/igor/Projects/locked");
    assert!(locked.items.is_empty(), "{locked:?}");
    assert_eq!(
        locked.blocked,
        BTreeMap::from([(
            PathBuf::from("/Users/igor/Projects/locked/node_modules"),
            DenyReason::ProtectedFlag("UF_IMMUTABLE"),
        )])
    );
}

#[test]
fn the_level_totals_count_project_artifacts_like_any_other_item() {
    let index = workspace();
    let (rules, recommendations) = recommendations(&index);
    let on_cards: u64 = recommendations
        .cards
        .iter()
        .filter(|card| rules.rule(card.rule).level == SafetyLevel::Safe && !rules.rule(card.rule).is_info_only())
        .map(|card| card.reclaimable.bytes)
        .sum();
    let in_projects: u64 = recommendations
        .projects
        .iter()
        .map(|project| project.reclaimable.bytes)
        .sum();
    assert!(in_projects > 0);
    assert_eq!(recommendations.totals.safe, on_cards + in_projects);
    assert_eq!(recommendations.totals.total(), recommendations.totals.safe);
}

#[test]
fn projects_are_ordered_by_what_they_free() {
    let index = workspace();
    let (_, recommendations) = recommendations(&index);
    assert!(
        recommendations
            .projects
            .is_sorted_by_key(|project| std::cmp::Reverse(project.reclaimable.bytes))
    );
}

#[test]
fn every_rule_that_finds_project_artifacts_is_safe() {
    let table = rule_table(&ConfigOverrides::default());
    let project_rules: Vec<_> = table
        .iter()
        .filter(|rule| {
            rule.paths()
                .iter()
                .any(|path| path.starts_with("**/") && path.contains('|'))
        })
        .collect();
    assert!(project_rules.len() >= 6);
    for rule in project_rules {
        assert_eq!(rule.level, SafetyLevel::Safe, "{}", rule.id);
    }
}

fn dependencies(project: &str) -> Node {
    directory(
        project,
        vec![file("package.json"), directory("node_modules", vec![file("index.js")])],
    )
}

#[test]
fn dependencies_of_installed_tools_are_not_projects() {
    let tools = [
        "/Applications/Visual Studio Code.app/Contents/Resources/app",
        "/Users/igor/.vscode/extensions/swift-2.18.0",
        "/Users/igor/Library/pnpm/global/5",
        "/Users/igor/.nvm/versions/node/v22.11.0/lib/node_modules/npm",
    ];
    let mut trees: Vec<Node> = tools
        .iter()
        .map(|tool| {
            let tool = Path::new(tool);
            path_tree(
                &tool.parent().expect("a tool sits in a folder").to_string_lossy(),
                dependencies(&tool.file_name().expect("a tool has a name").to_string_lossy()),
            )
        })
        .collect();
    trees.push(path_tree("/Users/igor/Projects", dependencies("web")));
    let index = data_volume_index(merge(trees));
    let (rules, recommendations) = recommendations(&index);

    let roots: BTreeSet<PathBuf> = projects(&index, &recommendations).into_keys().collect();

    assert_eq!(roots, paths(&["/Users/igor/Projects/web"]));
    for tool in tools {
        let dependencies = index
            .find(&Path::new(tool).join("node_modules"))
            .unwrap_or_else(|| panic!("{tool} is indexed"));
        assert_eq!(
            rules.claim_of(&index, dependencies),
            None,
            "the dependencies of {tool} belong to the tool"
        );
    }
}
