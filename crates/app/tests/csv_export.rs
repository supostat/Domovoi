use std::ffi::OsStr;
use std::path::Path;

use domovoi_app::csv_lines;
use domovoi_index::{Enrichment, Index, IndexRoot, NodeId};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const HEADER: &str = "path,bytes,category,modified";
const COMMA_NAME: &str = "sales, 2026.csv";
const QUOTE_NAME: &str = "say \"hi\".txt";
const ISO_LENGTH: usize = "2026-10-27T10:00:00Z".len();

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn depth_first(index: &Index, id: NodeId, order: &mut Vec<NodeId>) {
    order.push(id);
    for child in index.children(id) {
        depth_first(index, child, order);
    }
}

#[test]
fn the_export_lists_every_node_in_tree_order_with_escaped_fields() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file(COMMA_NAME, 2_048, Fill::Byte(1)).expect("file is written");
    root.file(QUOTE_NAME, 512, Fill::Byte(2)).expect("file is written");
    let notes = root.directory("notes").expect("directory is created");
    notes.file("plain.txt", 4_096, Fill::Noise(3)).expect("file is written");
    let index = index_of(tree.path());

    let lines = csv_lines(&index, index.root());

    assert!(lines.ends_with("\r\n"), "rows end with CRLF");
    let rows: Vec<&str> = lines.split("\r\n").filter(|row| !row.is_empty()).collect();
    assert_eq!(rows[0], HEADER);
    let mut order = Vec::new();
    depth_first(&index, index.root(), &mut order);
    assert_eq!(rows.len(), order.len() + 1, "one row per node after the header");
    for (row, id) in rows[1..].iter().zip(&order) {
        let record = index.node(*id);
        let modified = row.rsplit(',').next().expect("the row has columns");
        let dated = modified.len() == ISO_LENGTH && modified.ends_with('Z');
        assert!(
            dated || (modified.is_empty() && !record.modified.is_known()),
            "a known date is printed in UTC, an unknown one leaves the field empty: {row}"
        );
        let without_date = &row[..row.len() - modified.len() - 1];
        let category = without_date.rsplit(',').next().expect("the row has columns");
        assert_eq!(category, record.category.id(), "{row}");
        let without_category = &without_date[..without_date.len() - category.len() - 1];
        let bytes = without_category.rsplit(',').next().expect("the row has columns");
        assert_eq!(bytes, record.allocated.to_string(), "{row}");
        let path = &without_category[..without_category.len() - bytes.len() - 1];
        let expected = index.path(*id).to_string_lossy().into_owned();
        if expected.contains([',', '"']) {
            assert_eq!(path, format!("\"{}\"", expected.replace('"', "\"\"")), "{row}");
        } else {
            assert_eq!(path, expected, "{row}");
        }
    }
    assert!(
        rows[1..].iter().any(|row| row.ends_with('Z')),
        "the files carry their modification dates"
    );
    let comma = index
        .child_named(index.root(), OsStr::new(COMMA_NAME))
        .expect("the comma file is indexed");
    assert!(
        rows.iter().any(|row| row.starts_with(&format!(
            "\"{}\",{},",
            index.path(comma).display(),
            index.node(comma).allocated
        ))),
        "a path with a comma is quoted"
    );
    assert!(
        rows.iter().any(|row| row.contains("say \"\"hi\"\".txt\",")),
        "a quote inside a path is doubled"
    );
}
