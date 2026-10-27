use std::ffi::OsStr;
use std::path::Path;
use std::time::SystemTime;

use domovoi_app::IndexFacts;
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const NO_BREAK_SPACE: char = '\u{A0}';
const FILES: usize = 3;

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

#[test]
fn a_folder_reports_its_dates_and_how_many_files_it_holds() {
    let tree = TempTree::new().expect("temp tree is created");
    let notes = tree
        .root()
        .expect("root opens")
        .directory("notes")
        .expect("directory is created");
    for number in 0..FILES {
        notes
            .file(format!("note-{number}.txt"), 1_024, Fill::Byte(number as u8))
            .expect("file is written");
    }
    let index = index_of(tree.path());
    let folder = index
        .child_named(index.root(), OsStr::new("notes"))
        .expect("the folder is indexed");
    let file = index
        .child_named(folder, OsStr::new("note-0.txt"))
        .expect("the file is indexed");
    let now = SystemTime::now();

    let folder_facts = IndexFacts::of(&index, folder, now);
    let file_facts = IndexFacts::of(&index, file, now);

    assert_eq!(folder_facts.created.as_deref(), Some("создан сегодня"));
    assert_eq!(folder_facts.modified.as_deref(), Some("изменён сегодня"));
    assert_eq!(
        folder_facts.inside.as_deref(),
        Some(format!("внутри {FILES}{NO_BREAK_SPACE}объекта").as_str())
    );
    assert_eq!(file_facts.created.as_deref(), Some("создан сегодня"));
    assert_eq!(file_facts.modified.as_deref(), Some("изменён сегодня"));
    assert_eq!(file_facts.inside, None, "a file holds nothing");
}
