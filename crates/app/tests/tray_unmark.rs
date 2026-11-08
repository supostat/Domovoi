use std::path::{Path, PathBuf};

use domovoi_app::TrayModel;
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, OwnerEvidence};
use domovoi_rules::{AppRemoval, RemovalReason, Tray};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const BUILD_RULE: &str = "cargo.target";
const DEPENDENCIES_RULE: &str = "dev.deps";
const WEB_BUILD_RULE: &str = "dev.web-build";
const BUNDLE_ID: &str = "com.domovoi.tiny";

struct Marked {
    _tree: TempTree,
    index: Index,
    loose: NodeId,
    first_build: NodeId,
    second_build: NodeId,
    bundle: NodeId,
    container: NodeId,
    site_modules: NodeId,
    site_build: NodeId,
}

impl Marked {
    fn tray() -> (Self, Tray) {
        let tree = TempTree::new().expect("temp tree is created");
        for folder in [
            "loose",
            "first/target",
            "second/target",
            "Tiny.app",
            "Containers/com.domovoi.tiny",
        ] {
            let mut directory = tree.root().expect("root opens");
            for name in folder.split('/') {
                directory = directory.directory(name).expect("directory is created");
            }
            directory
                .file("payload.bin", 4_096, Fill::Noise(3))
                .expect("file is written");
        }
        let site = tree
            .root()
            .expect("root opens")
            .directory("site")
            .expect("directory is created");
        for artifact in ["node_modules", ".next"] {
            site.directory(artifact)
                .expect("directory is created")
                .file("payload.bin", 4_096, Fill::Noise(3))
                .expect("file is written");
        }
        let index = index_of(tree.path());
        let marked = Self {
            loose: find(&index, "loose"),
            first_build: find(&index, "first/target"),
            second_build: find(&index, "second/target"),
            bundle: find(&index, "Tiny.app"),
            container: find(&index, "Containers/com.domovoi.tiny"),
            site_modules: find(&index, "site/node_modules"),
            site_build: find(&index, "site/.next"),
            index,
            _tree: tree,
        };
        let mut tray = Tray::default();
        tray.add(&marked.index, marked.loose, None);
        tray.add(&marked.index, marked.first_build, Some(BUILD_RULE));
        tray.add(&marked.index, marked.second_build, Some(BUILD_RULE));
        tray.add_application(
            &marked.index,
            &AppRemoval {
                bundle_id: BUNDLE_ID.to_owned(),
                name: "Tiny".to_owned(),
                items: vec![
                    (marked.bundle, RemovalReason::Bundle),
                    (marked.container, RemovalReason::Owner(OwnerEvidence::ContainerMetadata)),
                ],
                name_only: Vec::new(),
            },
        );
        let site = tree_path(&marked.index, "site");
        tray.add_project_artifact(&marked.index, marked.site_modules, DEPENDENCIES_RULE, &site);
        tray.add_project_artifact(&marked.index, marked.site_build, WEB_BUILD_RULE, &site);
        (marked, tray)
    }

    fn model(&self, tray: &Tray) -> TrayModel {
        TrayModel::measured(&self.index, tray, &TrayModel::estimated_sizes(&self.index, tray))
    }
}

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn find(index: &Index, relative: &str) -> NodeId {
    relative
        .split('/')
        .try_fold(index.root(), |node, name| index.child_named(node, name.as_ref()))
        .unwrap_or_else(|| panic!("{relative} is indexed"))
}

fn tree_path(index: &Index, relative: &str) -> PathBuf {
    index.path(find(index, relative))
}

fn unmark_row(tray: &mut Tray, model: &TrayModel, row: usize) {
    for node in &model.row_nodes[row] {
        tray.remove(*node);
    }
}

#[test]
fn each_row_of_the_sheet_knows_the_nodes_it_stands_for() {
    let (marked, tray) = Marked::tray();

    let model = marked.model(&tray);

    assert_eq!(
        model.rows.len(),
        4,
        "a loose folder, one rule group, one application and one project"
    );
    assert_eq!(
        model.row_nodes,
        [
            vec![marked.loose],
            vec![marked.first_build, marked.second_build],
            vec![marked.bundle, marked.container],
            vec![marked.site_modules, marked.site_build],
        ]
    );
}

#[test]
fn unmarking_a_rule_row_takes_every_folder_of_the_rule_and_the_rest_stays() {
    let (marked, mut tray) = Marked::tray();
    let model = marked.model(&tray);

    unmark_row(&mut tray, &model, 1);

    let after = marked.model(&tray);
    assert_eq!(
        after.row_nodes,
        [
            vec![marked.loose],
            vec![marked.bundle, marked.container],
            vec![marked.site_modules, marked.site_build],
        ]
    );
    assert_eq!(after.rows[0], model.rows[0], "the loose folder keeps its row");
    assert_eq!(after.rows[1], model.rows[2], "the application keeps its row");
    assert_eq!(after.rows[2], model.rows[3], "the project keeps its row");
    assert_eq!(
        after.total(),
        model.rows[0].size + model.rows[2].size + model.rows[3].size
    );
}

#[test]
fn unmarking_an_application_row_drops_the_application_with_its_files() {
    let (marked, mut tray) = Marked::tray();
    let model = marked.model(&tray);

    unmark_row(&mut tray, &model, 2);

    assert!(tray.applications().is_empty(), "no application is left without files");
    assert!(
        tray.items().iter().all(|item| item.application.is_none()),
        "no file of the application stays marked"
    );
    assert_eq!(marked.model(&tray).rows.len(), 3);
}

#[test]
fn a_project_row_names_the_project_and_lists_its_artifacts() {
    let (marked, tray) = Marked::tray();

    let model = marked.model(&tray);

    let row = &model.rows[3];
    assert_eq!(row.title, "site");
    assert_eq!(row.details.len(), 2, "{:?}", row.details);
    assert!(row.details[0].ends_with("site/node_modules"), "{:?}", row.details);
    assert!(row.details[1].ends_with("site/.next"), "{:?}", row.details);
}

#[test]
fn unmarking_a_project_row_takes_every_artifact_of_the_project() {
    let (marked, mut tray) = Marked::tray();
    let model = marked.model(&tray);

    unmark_row(&mut tray, &model, 3);

    assert!(
        tray.items().iter().all(|item| item.project.is_none()),
        "no artifact of the project stays marked"
    );
    let after = marked.model(&tray);
    assert_eq!(after.rows.len(), 3);
    assert_eq!(after.rows[..], model.rows[..3]);
}

#[test]
fn unmarking_the_last_row_leaves_an_empty_sheet() {
    let (marked, mut tray) = Marked::tray();
    for _ in 0..4 {
        let model = marked.model(&tray);
        unmark_row(&mut tray, &model, 0);
    }

    let empty = marked.model(&tray);

    assert!(tray.items().is_empty());
    assert!(empty.rows.is_empty() && empty.row_nodes.is_empty());
    assert_eq!(empty.total(), 0);
}
