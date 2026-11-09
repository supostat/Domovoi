use std::ffi::OsStr;
use std::path::Path;

use domovoi_app::{DuplicateKey, GroupModel, MemberModel, PreviewTargets};
use domovoi_index::{Category, Enrichment, Index, IndexRoot, NodeId, Source};
use domovoi_render::{DiagramOutline, DiagramTree, NodeOrigin, ROOT};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};

const DEPTH: u32 = 3;

fn index_of(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(
        &report,
        IndexRoot::for_path(root).expect("root kind"),
        &Enrichment::default(),
    )
}

fn node_at(index: &Index, relative: &str) -> NodeId {
    Path::new(relative).components().fold(index.root(), |parent, part| {
        index
            .child_named(parent, part.as_os_str())
            .expect("the file is indexed")
    })
}

fn member(index: &Index, relative: &str) -> MemberModel {
    MemberModel {
        path: format!("~/{relative}"),
        size: 4_096,
        frees: 4_096,
        clone: false,
        checked: false,
        refusal: None,
        package: None,
        node: Some(node_at(index, relative)),
    }
}

#[test]
fn level_targets_are_the_indexed_paths_of_the_selection() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    root.file("photo.heic", 8_192, Fill::Noise(1)).expect("file is written");
    root.file("notes.txt", 1_024, Fill::Byte(2)).expect("file is written");
    let index = index_of(tree.path());
    let diagram = DiagramTree::from_index(&index, index.root(), DEPTH);
    let children = diagram.node(ROOT).children.clone();
    assert_eq!(children.len(), 2);

    let targets = PreviewTargets::from_level(&diagram, &index, &children);
    let photo = index
        .child_named(index.root(), OsStr::new("photo.heic"))
        .expect("the photo is indexed");
    let notes = index
        .child_named(index.root(), OsStr::new("notes.txt"))
        .expect("the notes are indexed");
    let mut expected = vec![index.path(photo), index.path(notes)];
    let mut found = targets.paths.clone();
    expected.sort();
    found.sort();
    assert_eq!(found, expected);
    assert!(PreviewTargets::from_level(&diagram, &index, &[]).is_empty());

    let mixed = DiagramTree::from_outline(&DiagramOutline {
        name: "Macintosh HD".to_owned(),
        category: Category::Home,
        size: 0,
        origin: NodeOrigin::Outline,
        children: vec![
            DiagramOutline {
                name: "photo.heic".to_owned(),
                category: Category::Media,
                size: 8_192,
                origin: NodeOrigin::Indexed(photo),
                children: Vec::new(),
            },
            DiagramOutline {
                name: "macOS".to_owned(),
                category: Category::System,
                size: 1_000,
                origin: NodeOrigin::Measured(Source::Getattrlist),
                children: Vec::new(),
            },
        ],
    });
    let mixed_children = mixed.node(ROOT).children.clone();
    let targets = PreviewTargets::from_level(&mixed, &index, &mixed_children);
    assert_eq!(
        targets.paths,
        [index.path(photo)],
        "a measured node has no path and is skipped"
    );
}

#[test]
fn duplicate_targets_come_from_member_rows_only() {
    let tree = TempTree::new().expect("temp tree is created");
    let root = tree.root().expect("root opens");
    for (folder, names) in [
        ("Pictures", &["IMG_0412.jpg"][..]),
        ("Downloads", &["IMG_0412.jpg", "report.pdf"]),
        ("Desktop", &["IMG_0412 (1).jpg"]),
        ("Documents", &["report.pdf"]),
    ] {
        let directory = root.directory(folder).expect("a folder is created");
        for name in names {
            directory.file(name, 4_096, Fill::Byte(3)).expect("file is written");
        }
    }
    let index = index_of(tree.path());
    let groups = vec![
        GroupModel {
            name: "IMG_0412.jpg".to_owned(),
            keep: member(&index, "Pictures/IMG_0412.jpg"),
            copies: vec![
                member(&index, "Downloads/IMG_0412.jpg"),
                member(&index, "Desktop/IMG_0412 (1).jpg"),
            ],
        },
        GroupModel {
            name: "report.pdf".to_owned(),
            keep: member(&index, "Documents/report.pdf"),
            copies: vec![member(&index, "Downloads/report.pdf")],
        },
    ];
    let keys = [
        DuplicateKey::parse("0"),
        DuplicateKey::parse("0.0"),
        DuplicateKey::parse("0.2"),
        DuplicateKey::parse("1.1"),
    ];

    let targets = PreviewTargets::from_duplicates(&groups, &index, &keys);

    assert_eq!(
        targets.paths,
        [
            tree.path().join("Pictures/IMG_0412.jpg"),
            tree.path().join("Desktop/IMG_0412 (1).jpg"),
            tree.path().join("Downloads/report.pdf"),
        ],
        "a group header is not a target, members are, by their indexed path"
    );
    assert!(PreviewTargets::from_duplicates(&groups, &index, &[]).is_empty());
    assert_eq!(DuplicateKey::parse("1.1").text(), "1.1");
    assert_eq!(DuplicateKey::parse("3"), DuplicateKey { group: 3, member: None });
}
