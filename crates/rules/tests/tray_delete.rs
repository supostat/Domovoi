use std::path::Path;

use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, Reclaimable};
use domovoi_rules::{CaseRule, DeleteRefusal, DeleteSubject, DenyReason, Denylist, Tray, delete};
use domovoi_scanner::{CancelToken, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const VOLUME_SERVICE_FOLDER: &str = ".Trashes";
const INSTALLER_RULE: &str = "downloads.installers";
const BUILD_FILES: u64 = 3;

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn child(index: &Index, name: &str) -> NodeId {
    index
        .child_named(index.root(), name.as_ref())
        .unwrap_or_else(|| panic!("{name} is indexed"))
}

fn measured(index: &Index, tray: &Tray) -> Vec<Reclaimable> {
    tray.items()
        .iter()
        .map(|item| index.reclaimable(item.node).expect("the item is measured"))
        .collect()
}

#[test]
fn taking_the_tray_separates_denied_items_from_subjects() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("report.pdf", 8_192, Fill::Noise(1)).expect("file is written");
    root.directory(VOLUME_SERVICE_FOLDER)
        .expect("directory is created")
        .file("leftover", 512, Fill::Byte(3))
        .expect("file is written");
    let index = index_of(tree.path());
    let report = child(&index, "report.pdf");
    let service = child(&index, VOLUME_SERVICE_FOLDER);
    let mut tray = Tray::default();
    tray.add(&index, report, Some(INSTALLER_RULE));
    tray.add(&index, service, None);
    let sizes = measured(&index, &tray);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");

    let (subjects, refusals) = tray.take_for_delete(&index, &denylist, &sizes, |_| false);

    assert!(tray.items().is_empty(), "the tray is emptied by taking");
    assert_eq!(
        subjects,
        [DeleteSubject {
            path: index.path(report),
            physical: index.physical_path(report),
            link_id: index.node(report).link_id,
            measured_bytes: sizes[0].bytes,
            indexed_files: 1,
            rule_id: Some(INSTALLER_RULE.to_owned()),
            duplicate_of: None,
        }]
    );
    assert_eq!(
        refusals,
        [DeleteRefusal {
            path: index.path(service),
            reason: DenyReason::ProtectedData("служебные данные тома"),
        }]
    );
    assert!(sizes[0].bytes > 0, "a measured subject carries its size");
}

#[test]
fn taken_subjects_delete_while_denied_items_stay() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("report.pdf", 8_192, Fill::Noise(1)).expect("file is written");
    root.directory(VOLUME_SERVICE_FOLDER)
        .expect("directory is created")
        .file("leftover", 512, Fill::Byte(3))
        .expect("file is written");
    let index = index_of(tree.path());
    let mut tray = Tray::default();
    tray.add(&index, child(&index, "report.pdf"), None);
    tray.add(&index, child(&index, VOLUME_SERVICE_FOLDER), None);
    let sizes = measured(&index, &tray);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");
    let (subjects, refusals) = tray.take_for_delete(&index, &denylist, &sizes, |_| false);

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(report.deleted, subjects);
    assert_eq!(report.freed_bytes, subjects[0].measured_bytes);
    assert!(!tree.path().join("report.pdf").exists());
    assert!(
        tree.path().join(VOLUME_SERVICE_FOLDER).join("leftover").exists(),
        "a denied item is never touched: {refusals:?}"
    );
}

#[test]
fn a_taken_folder_carries_the_number_of_its_files_from_the_index() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let build = root.directory("target").expect("directory is created");
    let nested = build.directory("debug").expect("directory is created");
    build.file("cache.bin", 1_024, Fill::Byte(4)).expect("file is written");
    for name in ["app", "app.d"] {
        nested.file(name, 2_048, Fill::Noise(2)).expect("file is written");
    }
    let index = index_of(tree.path());
    let mut tray = Tray::default();
    tray.add(&index, child(&index, "target"), None);
    let sizes = measured(&index, &tray);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");

    let (subjects, _) = tray.take_for_delete(&index, &denylist, &sizes, |_| false);

    assert_eq!(
        subjects[0].indexed_files, BUILD_FILES,
        "the files of the subtree, the folders are not counted"
    );
}

#[test]
fn the_tray_covers_an_item_and_its_inside_but_not_a_neighbour_sharing_the_start_of_its_name() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    let marked = root.directory("b").expect("directory is created");
    marked.file("inner", 512, Fill::Byte(5)).expect("file is written");
    root.directory("bc")
        .expect("directory is created")
        .file("other", 512, Fill::Byte(6))
        .expect("file is written");
    let index = index_of(tree.path());
    let folder = child(&index, "b");
    let neighbour = child(&index, "bc");
    let inner = index
        .child_named(folder, "inner".as_ref())
        .expect("the inner file is indexed");
    let mut tray = Tray::default();
    tray.add(&index, folder, None);

    assert!(tray.covers(&index.path(folder)), "the marked folder itself is covered");
    assert!(
        tray.covers(&index.path(inner)),
        "a path inside the marked folder is covered"
    );
    assert!(
        !tray.covers(&index.path(neighbour)),
        "a neighbour whose name starts the same is not inside the marked folder"
    );
}

#[test]
fn an_item_whose_rule_belongs_to_a_running_program_is_refused() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("report.pdf", 8_192, Fill::Noise(1)).expect("file is written");
    let index = index_of(tree.path());
    let report = child(&index, "report.pdf");
    let mut tray = Tray::default();
    tray.add(&index, report, Some(INSTALLER_RULE));
    let sizes = measured(&index, &tray);
    let denylist = Denylist::new(CaseRule::Insensitive).expect("denylist is prepared");

    let (subjects, refusals) = tray.take_for_delete(&index, &denylist, &sizes, |rule| rule == INSTALLER_RULE);

    assert!(subjects.is_empty(), "{subjects:?}");
    assert_eq!(
        refusals,
        [DeleteRefusal {
            path: index.path(report),
            reason: DenyReason::OwnerRunning,
        }]
    );
}
