use std::fs;
use std::path::Path;

use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, OwnerEvidence};
use domovoi_rules::{AppRemoval, RemovalReason, Tray, TrayApplication, TrayItem};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const PROJECT: &str = "condoit";
const PROJECT_BUILD: &str = "condoit/.build";
const PROJECT_RULE: &str = "dev.swiftpm";
const APPLICATION_NAME: &str = "Notes";
const BUNDLE_ID: &str = "com.example.notes";
const APPLICATION_BUNDLE: &str = "Applications/Notes.app";
const APPLICATION_CACHE: &str = "Caches/com.example.notes";
const APPLICATION_NAME_ONLY: &str = "Support/Notes";
const INSTALLER: &str = "Downloads/Notes.dmg";
const INSTALLER_RULE: &str = "downloads.installers";

fn volume() -> TempTree {
    let tree = TempTree::new().expect("a temp tree is created");
    let root = tree.root().expect("the root opens");
    let project = root.directory(PROJECT).expect("a project is created");
    project
        .file("Package.swift", 64, Fill::Byte(1))
        .expect("a manifest is written");
    project
        .directory(".build")
        .expect("a build folder is created")
        .file("build.o", 4_096, Fill::Noise(2))
        .expect("build output is written");
    root.directory("Applications")
        .expect("a folder is created")
        .directory("Notes.app")
        .expect("a bundle is created")
        .file("Notes", 4_096, Fill::Noise(3))
        .expect("an executable is written");
    root.directory("Caches")
        .expect("a folder is created")
        .directory(BUNDLE_ID)
        .expect("a cache is created")
        .file("cache.db", 1_024, Fill::Byte(4))
        .expect("a cache is written");
    root.directory("Support")
        .expect("a folder is created")
        .directory(APPLICATION_NAME)
        .expect("a support folder is created")
        .file("settings", 64, Fill::Byte(5))
        .expect("settings are written");
    root.directory("Downloads")
        .expect("a folder is created")
        .file("Notes.dmg", 8_192, Fill::Noise(6))
        .expect("an installer is written");
    root.directory("notes")
        .expect("a folder is created")
        .file("todo.txt", 64, Fill::Byte(7))
        .expect("a file is written");
    tree
}

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn node_at(index: &Index, relative: &str) -> NodeId {
    Path::new(relative).iter().fold(index.root(), |node, name| {
        index
            .child_named(node, name)
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    })
}

fn marked_tray(index: &Index) -> Tray {
    let mut tray = Tray::default();
    tray.add_project_artifact(
        index,
        node_at(index, PROJECT_BUILD),
        PROJECT_RULE,
        &index.path(node_at(index, PROJECT)),
    );
    tray.add_application(
        index,
        &AppRemoval {
            bundle_id: BUNDLE_ID.to_owned(),
            name: APPLICATION_NAME.to_owned(),
            items: vec![
                (node_at(index, APPLICATION_BUNDLE), RemovalReason::Bundle),
                (
                    node_at(index, APPLICATION_CACHE),
                    RemovalReason::Owner(OwnerEvidence::BundleIdName),
                ),
            ],
            name_only: vec![node_at(index, APPLICATION_NAME_ONLY)],
        },
    );
    tray.add(index, node_at(index, INSTALLER), Some(INSTALLER_RULE));
    tray
}

fn changed_on_disk(tree: &TempTree) -> Index {
    fs::write(tree.path().join("notes").join("fresh.txt"), b"a change on the volume").expect("a file appears");
    index_of(tree.path())
}

fn located(index: &Index, relative: &str) -> TrayItem {
    let node = node_at(index, relative);
    TrayItem {
        node,
        path: index.path(node),
        physical: index.physical_path(node),
        rule_id: None,
        reason: None,
        application: None,
        project: None,
        duplicate_of: None,
    }
}

#[test]
fn a_remapped_tray_keeps_the_project_the_application_with_its_reasons_and_the_rule() {
    let tree = volume();
    let tray = marked_tray(&index_of(tree.path()));
    let after = changed_on_disk(&tree);

    let remapped = tray.remapped(&after, |_, path| after.find(path));

    assert_eq!(
        remapped.items(),
        [
            TrayItem {
                rule_id: Some(PROJECT_RULE.to_owned()),
                project: Some(after.path(node_at(&after, PROJECT))),
                ..located(&after, PROJECT_BUILD)
            },
            TrayItem {
                reason: Some(RemovalReason::Bundle),
                application: Some(BUNDLE_ID.to_owned()),
                ..located(&after, APPLICATION_BUNDLE)
            },
            TrayItem {
                reason: Some(RemovalReason::Owner(OwnerEvidence::BundleIdName)),
                application: Some(BUNDLE_ID.to_owned()),
                ..located(&after, APPLICATION_CACHE)
            },
            TrayItem {
                rule_id: Some(INSTALLER_RULE.to_owned()),
                ..located(&after, INSTALLER)
            },
        ],
        "every item is found anew and keeps its rule, reason, application and project"
    );
    assert_eq!(
        remapped.applications(),
        [TrayApplication {
            bundle_id: BUNDLE_ID.to_owned(),
            name: APPLICATION_NAME.to_owned(),
            name_only: vec![after.path(node_at(&after, APPLICATION_NAME_ONLY))],
        }],
        "the application stays in the tray with its name-only folders"
    );
}

#[test]
fn an_application_whose_items_are_gone_leaves_the_list_of_applications() {
    let tree = volume();
    let tray = marked_tray(&index_of(tree.path()));
    for relative in [APPLICATION_BUNDLE, APPLICATION_CACHE] {
        fs::remove_dir_all(tree.path().join(relative)).expect("an item of the application is removed");
    }
    let after = changed_on_disk(&tree);

    let remapped = tray.remapped(&after, |_, path| after.find(path));

    assert!(
        remapped.applications().is_empty(),
        "an application with no item left is not listed"
    );
    assert_eq!(
        remapped
            .items()
            .iter()
            .map(|item| item.path.clone())
            .collect::<Vec<_>>(),
        [
            after.path(node_at(&after, PROJECT_BUILD)),
            after.path(node_at(&after, INSTALLER))
        ],
        "the items that are still on the volume stay"
    );
}
