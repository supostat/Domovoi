use std::path::Path;

use domovoi_index::{Enrichment, Index, IndexRoot, NodeId, Reclaimable};
use domovoi_rules::{CaseRule, DeleteRefusal, DeleteSubject, DenyReason, Denylist, Tray, delete};
use domovoi_scanner::{CancelToken, ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const VOLUME_SERVICE_FOLDER: &str = ".Trashes";
const INSTALLER_RULE: &str = "downloads.installers";

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

    let (subjects, refusals) = tray.take_for_delete(&index, &denylist, &sizes);

    assert!(tray.items().is_empty(), "the tray is emptied by taking");
    assert_eq!(
        subjects,
        [DeleteSubject {
            path: index.path(report),
            physical: index.physical_path(report),
            link_id: index.node(report).link_id,
            measured_bytes: sizes[0].bytes,
            rule_id: Some(INSTALLER_RULE.to_owned()),
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
    let (subjects, refusals) = tray.take_for_delete(&index, &denylist, &sizes);

    let report = delete(subjects.clone(), &CancelToken::default(), &|_| {});

    assert_eq!(report.deleted, subjects);
    assert_eq!(report.freed_bytes, subjects[0].measured_bytes);
    assert!(!tree.path().join("report.pdf").exists());
    assert!(
        tree.path().join(VOLUME_SERVICE_FOLDER).join("leftover").exists(),
        "a denied item is never touched: {refusals:?}"
    );
}
