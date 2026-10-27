use std::path::{Path, PathBuf};

use domovoi_app::{HoverTip, OutlineFixture, Screen, ShellFixture, strings};
use domovoi_index::{Category, Source};
use domovoi_render::{DiagramOutline, DiagramTree, NodeOrigin, ROOT};

const MUSIC: &str = "Music";
const NESTED_MUSIC: &str = "Music ▸ Music";
const GIGABYTE: u64 = 1_000_000_000;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn child_named(tree: &DiagramTree, parent: usize, name: &str) -> usize {
    tree.node(parent)
        .children
        .iter()
        .copied()
        .find(|child| tree.node(*child).name == name)
        .unwrap_or_else(|| panic!("{name} is a child of {}", tree.node(parent).name))
}

fn measured(name: &str, size: u64, children: Vec<DiagramOutline>) -> DiagramOutline {
    DiagramOutline {
        name: name.to_owned(),
        category: Category::Media,
        size,
        origin: NodeOrigin::Measured(Source::Scan),
        children,
    }
}

#[test]
fn a_folder_named_like_its_parent_shows_the_parent_before_its_name() {
    let mut fixture = ShellFixture::read(&fixture()).expect("the fixture reads");
    fixture.tree.children.push(OutlineFixture {
        name: MUSIC.to_owned(),
        category: "media".to_owned(),
        gigabytes: None,
        children: vec![
            OutlineFixture {
                name: MUSIC.to_owned(),
                category: "media".to_owned(),
                gigabytes: Some(2.0),
                children: Vec::new(),
            },
            OutlineFixture {
                name: "Podcasts".to_owned(),
                category: "media".to_owned(),
                gigabytes: Some(1.0),
                children: Vec::new(),
            },
        ],
    });
    let data = fixture
        .shell_data(Screen::Main)
        .expect("the fixture builds the screen data");
    let outer = child_named(&data.tree, ROOT, MUSIC);
    let inner = child_named(&data.tree, outer, MUSIC);

    let top_rows = data.level_rows(ROOT);
    let outer_row = top_rows
        .iter()
        .find(|row| row.node == outer)
        .expect("the outer folder is a row");
    assert_eq!(
        outer_row.name, MUSIC,
        "a folder with a differently named parent keeps its name"
    );
    for row in &top_rows {
        assert_eq!(row.name, data.tree.node(row.node).name);
    }

    let inner_rows = data.level_rows(outer);
    let names: Vec<&str> = inner_rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, [NESTED_MUSIC, "Podcasts"]);
    assert_eq!(
        data.tree.display_name(inner, strings::DIAGRAM.nested_separator),
        NESTED_MUSIC
    );
    assert_eq!(
        data.crumbs(inner).last().map(|crumb| crumb.title.as_str()),
        Some(MUSIC),
        "crumbs already show the parent"
    );
}

#[test]
fn measured_nodes_get_the_same_treatment_in_the_hover_tip() {
    let mut data = ShellFixture::read(&fixture())
        .and_then(|fixture| fixture.shell_data(Screen::Main))
        .expect("the fixture builds the screen data");
    data.tree = DiagramTree::from_outline(&measured(
        "Macintosh HD",
        0,
        vec![measured(
            MUSIC,
            0,
            vec![
                measured(MUSIC, 3 * GIGABYTE, Vec::new()),
                measured("Podcasts", GIGABYTE, Vec::new()),
            ],
        )],
    ));
    let outer = child_named(&data.tree, ROOT, MUSIC);
    let inner = child_named(&data.tree, outer, MUSIC);
    let tip = HoverTip::for_node(&data, None, inner).expect("a measured node has a tip");
    assert_eq!(tip.name, NESTED_MUSIC);
    assert_eq!(tip.source, strings::SOURCE_SCAN);
    let outer_tip = HoverTip::for_node(&data, None, outer).expect("a measured node has a tip");
    assert_eq!(outer_tip.name, MUSIC);
}
