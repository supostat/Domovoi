mod synthetic_tree;

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use domovoi_index::{
    Badges, Category, Enrichment, Index, IndexNode, IndexRoot, ModelEvent, NodeId, Query, SafetyLevel, SafetyLevels,
};
use domovoi_scanner::{Node, ScanOptions, ScanReport, scan};
use domovoi_testkit::{Fill, TempTree};
use synthetic_tree::{data_volume, directory, file, opened_file};

const NODES_ON_A_FULL_DISK: usize = 5_650_000;
const MEMORY_BUDGET_BYTES: usize = 900_000_000;

struct NoLevels;

impl SafetyLevels for NoLevels {
    fn level(&self, _index: &Index, _node: NodeId) -> Option<SafetyLevel> {
        None
    }
}

struct DerivedDataIsSafe;

impl SafetyLevels for DerivedDataIsSafe {
    fn level(&self, index: &Index, node: NodeId) -> Option<SafetyLevel> {
        (index.name(node) == "DerivedData").then_some(SafetyLevel::Safe)
    }
}

fn indexed_temp_tree(tree: &TempTree, options: &ScanOptions) -> Index {
    let report = scan(tree.path(), options).expect("temp tree scans");
    let root = IndexRoot::for_path(tree.path()).expect("root kind is known");
    Index::build(&report, root, &Enrichment::default())
}

fn at<'index>(index: &'index Index, relative: &str) -> (NodeId, &'index IndexNode) {
    let mut node = index.root();
    for name in Path::new(relative).components() {
        node = index
            .child_named(node, name.as_os_str())
            .unwrap_or_else(|| panic!("{relative} is indexed"));
    }
    (node, index.node(node))
}

fn reverse_children(node: &mut Node) {
    node.children.reverse();
    for child in &mut node.children {
        reverse_children(child);
    }
}

fn synthetic_disk() -> (ScanReport, IndexRoot) {
    data_volume(vec![
        directory(
            "Users",
            vec![directory(
                "igor",
                vec![
                    directory(
                        "Library",
                        vec![
                            directory("Caches", vec![directory("Homebrew", vec![file("bottle.tar.gz", 300)])]),
                            directory("Developer", vec![directory("DerivedData", vec![file("index.db", 900)])]),
                            directory(
                                "Application Support",
                                vec![directory(
                                    "com.domovoi.app",
                                    vec![directory("models", vec![file("qwen.gguf", 700)])],
                                )],
                            ),
                        ],
                    ),
                    directory("Pictures", vec![file("holiday.heic", 50)]),
                    directory("Downloads", vec![file("installer.dmg", 120)]),
                    directory(
                        "Projects",
                        vec![directory(
                            "engram",
                            vec![file("Cargo.toml", 1), directory("target", vec![file("app", 400)])],
                        )],
                    ),
                    file("scan.png", 20),
                    file("notes", 2),
                ],
            )],
        ),
        directory("Applications", vec![directory("Notes.app", vec![file("Notes", 30)])]),
        directory("Library", vec![directory("Audio", vec![file("loop.caf", 10)])]),
        directory(".Spotlight-V100", vec![file("store.db", 40)]),
    ])
}

#[test]
fn index_nodes_fit_the_memory_budget_of_a_full_disk() {
    assert!(
        size_of::<IndexNode>() * NODES_ON_A_FULL_DISK <= MEMORY_BUDGET_BYTES,
        "{} bytes per node",
        size_of::<IndexNode>()
    );
}

#[test]
fn categories_follow_paths_project_markers_and_file_types() {
    let (report, root) = synthetic_disk();
    let index = Index::build(&report, root, &Enrichment::default());
    let expectations = [
        ("Users/igor", Category::Home),
        ("Users/igor/Library/Caches/Homebrew", Category::Cache),
        ("Users/igor/Library/Developer/DerivedData", Category::Dev),
        (
            "Users/igor/Library/Application Support/com.domovoi.app/models/qwen.gguf",
            Category::Model,
        ),
        ("Users/igor/Pictures/holiday.heic", Category::Media),
        ("Users/igor/Downloads/installer.dmg", Category::Docs),
        ("Users/igor/Projects", Category::Home),
        ("Users/igor/Projects/engram/target/app", Category::Dev),
        ("Users/igor/scan.png", Category::Media),
        ("Users/igor/notes", Category::Home),
        ("Applications/Notes.app", Category::Apps),
        ("Library/Audio", Category::System),
        (".Spotlight-V100", Category::System),
    ];
    for (relative, category) in expectations {
        assert_eq!(at(&index, relative).1.category, category, "{relative}");
    }
}

#[test]
fn model_events_move_downloaded_weights_into_the_model_category() {
    let (report, root) = data_volume(vec![directory(
        "Users",
        vec![directory(
            "igor",
            vec![directory("Weights", vec![file("t-lite.gguf", 500)])],
        )],
    )]);
    let mut enrichment = Enrichment::default();
    let mut index = Index::build(&report, root, &enrichment);
    let weights = PathBuf::from("/Users/igor/Weights/t-lite.gguf");
    index.apply_model_event(&ModelEvent::Downloaded { path: weights.clone() }, &mut enrichment);
    assert_eq!(at(&index, "Users/igor/Weights/t-lite.gguf").1.category, Category::Model);
    index.apply_model_event(&ModelEvent::Removed { path: weights }, &mut enrichment);
    assert_eq!(at(&index, "Users/igor/Weights/t-lite.gguf").1.category, Category::Home);
}

#[test]
fn pnpm_store_keeps_the_size_and_project_clones_share_its_blocks() {
    let tree = TempTree::new().expect("temp tree is created");
    tree.root()
        .expect("root opens")
        .pnpm_store(3)
        .expect("pnpm store is created");
    let index = indexed_temp_tree(&tree, &ScanOptions::default());
    let (_, canonical) = at(&index, "pnpm-store/content-0");
    assert!(canonical.badges.contains(Badges::CLONE_CANONICAL));
    assert_eq!(canonical.allocated, canonical.alloc_size);
    assert_eq!(canonical.shares_with, 3);
    for project in 0..3 {
        let relative = format!("project-{project}/node_modules/.pnpm/left-pad@1.3.0/node_modules/left-pad/index.js");
        let (_, clone) = at(&index, &relative);
        assert!(clone.badges.contains(Badges::CLONE), "{relative}");
        assert_eq!((clone.allocated, clone.shares_with), (0, 3), "{relative}");
    }
}

#[test]
fn hard_links_count_once_and_carry_a_badge() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("a-original", 40_000, Fill::Noise(1))
        .expect("file is written");
    root.hardlink("a-original", "b-link").expect("link is created");
    let index = indexed_temp_tree(&tree, &ScanOptions::default());
    let (_, original) = at(&index, "a-original");
    let (_, link) = at(&index, "b-link");
    assert!(original.badges.contains(Badges::HARD_LINK) && link.badges.contains(Badges::HARD_LINK));
    assert_eq!(original.allocated + link.allocated, original.alloc_size);
    assert_eq!(index.node(index.root()).allocated, original.alloc_size);
}

#[test]
fn totals_do_not_depend_on_worker_count_or_child_order() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.pnpm_store(2).expect("pnpm store is created");
    root.file("noise", 90_000, Fill::Noise(7)).expect("file is written");
    root.hardlink("noise", "noise-link").expect("link is created");
    let single = ScanOptions {
        workers: NonZeroUsize::new(1),
        ..ScanOptions::default()
    };
    let many = ScanOptions {
        workers: NonZeroUsize::new(8),
        ..ScanOptions::default()
    };
    let root_kind = IndexRoot::for_path(tree.path()).expect("root kind is known");
    let single_report = scan(tree.path(), &single).expect("single-worker scan");
    let mut many_report = scan(tree.path(), &many).expect("many-worker scan");
    reverse_children(&mut many_report.root);
    let single_index = Index::build(&single_report, root_kind.clone(), &Enrichment::default());
    let many_index = Index::build(&many_report, root_kind, &Enrichment::default());
    assert_eq!(single_index, many_index);
}

#[test]
fn queries_filter_by_category_level_size_inactivity_and_prefix() {
    let (report, root) = synthetic_disk();
    let index = Index::build(&report, root, &Enrichment::default());
    let path_of = |nodes: Vec<NodeId>| nodes.into_iter().map(|node| index.path(node)).collect::<Vec<_>>();
    let caches = Query {
        category: Some(Category::Cache),
        ..Query::default()
    };
    assert_eq!(
        path_of(index.query(&caches, &NoLevels)),
        [PathBuf::from("/Users/igor/Library/Caches")]
    );
    let safe = Query {
        level: Some(SafetyLevel::Safe),
        ..Query::default()
    };
    assert_eq!(
        path_of(index.query(&safe, &DerivedDataIsSafe)),
        [PathBuf::from("/Users/igor/Library/Developer/DerivedData")]
    );
    let large_in_projects = Query {
        min_allocated: 400 * 4096,
        path_prefix: Some(PathBuf::from("/Users/igor/Projects")),
        ..Query::default()
    };
    assert_eq!(
        path_of(index.query(&large_in_projects, &NoLevels)),
        [PathBuf::from("/Users/igor/Projects")]
    );
    let too_large = Query {
        min_allocated: u64::MAX,
        ..Query::default()
    };
    assert!(index.query(&too_large, &NoLevels).is_empty());
}

#[test]
fn inactivity_uses_the_newest_opening_or_change_in_a_subtree() {
    let long_ago = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_500_000_000);
    let recently = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_760_000_000);
    let (report, root) = data_volume(vec![directory(
        "Users",
        vec![directory(
            "igor",
            vec![
                directory("Old", vec![opened_file("letter", 1, long_ago)]),
                directory("Fresh", vec![opened_file("draft", 1, recently)]),
            ],
        )],
    )]);
    let index = Index::build(&report, root, &Enrichment::default());
    let stale = Query {
        inactive_since: Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_750_000_000)),
        path_prefix: Some(PathBuf::from("/Users/igor/Fresh")),
        ..Query::default()
    };
    assert!(index.query(&stale, &NoLevels).is_empty());
    let stale_in_home: Vec<_> = index
        .query(
            &Query {
                path_prefix: Some(PathBuf::from("/Users/igor")),
                ..stale
            },
            &NoLevels,
        )
        .into_iter()
        .map(|node| index.path(node))
        .collect();
    assert_eq!(stale_in_home, [PathBuf::from("/Users/igor/Old")]);
}
