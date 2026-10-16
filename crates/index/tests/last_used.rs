use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domovoi_index::{Enrichment, Index, IndexRoot, LastOpenedSource, NodeId, Timestamp};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const NEXT_SECOND: Duration = Duration::from_millis(1_100);

fn build(tree: &TempTree, enrichment: &Enrichment) -> Index {
    let report = scan(tree.path(), &ScanOptions::default()).expect("temp tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(tree.path()).expect("root kind"),
        enrichment,
    )
}

fn node(index: &Index, name: &str) -> NodeId {
    index
        .child_named(index.root(), name.as_ref())
        .unwrap_or_else(|| panic!("{name} is indexed"))
}

#[test]
fn files_take_their_date_from_the_xattr_and_copies_inherit_it() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("original", 4_096, Fill::Byte(1)).expect("file is written");
    let opened = SystemTime::now();
    root.last_used("original", opened).expect("last used date is set");
    thread::sleep(NEXT_SECOND);
    root.clone_of("original", "copy").expect("copy is created");
    root.file("never-opened", 4_096, Fill::Byte(2))
        .expect("file is written");

    let index = build(&tree, &Enrichment::default());
    let original = index.node(node(&index, "original"));
    assert_eq!(original.last_opened, Timestamp::from_system_time(opened));
    assert_eq!(original.last_opened_source, LastOpenedSource::Xattr);
    let copy = index.node(node(&index, "copy"));
    assert_eq!(copy.last_opened, original.last_opened);
    assert_eq!(copy.last_opened_source, LastOpenedSource::Inherited);
    let never = index.node(node(&index, "never-opened"));
    assert_eq!(never.last_opened_source, LastOpenedSource::Unknown);
    assert!(!never.last_opened.is_known());
}

#[test]
fn directories_and_bundles_take_their_date_from_spotlight() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.directory("Tool.app").expect("bundle is created");
    root.file("loose", 10, Fill::Byte(3)).expect("file is written");
    let canonical: PathBuf = std::fs::canonicalize(tree.path()).expect("tree canonicalizes");
    let opened = UNIX_EPOCH + Duration::from_secs(1_740_000_000);
    let enrichment = Enrichment {
        spotlight_last_used: vec![(canonical.join("Tool.app"), opened), (canonical.join("loose"), opened)],
        ..Enrichment::default()
    };
    let index = build(&tree, &enrichment);
    let bundle = index.node(node(&index, "Tool.app"));
    assert_eq!(bundle.last_opened, Timestamp::from_system_time(opened));
    assert_eq!(bundle.last_opened_source, LastOpenedSource::SpotlightIndex);
    assert_eq!(
        index.node(index.root()).newest_opened,
        Timestamp::from_system_time(opened)
    );
    let loose = index.node(node(&index, "loose"));
    assert_eq!(loose.last_opened_source, LastOpenedSource::Unknown);
}

#[test]
fn spotlight_channel_runs_one_query_per_attribute_set() {
    let enrichment = Enrichment::gather(Vec::new()).expect("Spotlight queries run");
    assert!(
        enrichment
            .spotlight_last_used
            .iter()
            .all(|(path, _)| path.is_absolute())
    );
}
