mod support;

use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_render::{DiagramPainter, DiagramTree, GlyphAtlas, ROOT, SunburstLayout, category_color};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{Fill, TempTree};
use domovoi_theme::Palette;
use support::{STRINGS, crate_path, fonts, renderer};

const SIDE: f32 = 360.0;
const DEPTH: u32 = 4;
const CHANNEL_TOLERANCE: u8 = 2;

#[test]
fn the_real_palette_and_a_real_index_produce_a_frame() {
    fonts();
    let tree_on_disk = TempTree::new().expect("a temp tree is created");
    let root = tree_on_disk.root().expect("the root opens");
    let engine = root.directory("engine").expect("a directory is created");
    engine.file("Cargo.toml", 64, Fill::Byte(1)).expect("a file is written");
    engine
        .directory("target")
        .expect("a directory is created")
        .file("engine.bin", 400_000, Fill::Noise(1))
        .expect("a file is written");
    root.directory("Movies")
        .expect("a directory is created")
        .file("holiday.mov", 250_000, Fill::Noise(2))
        .expect("a file is written");
    root.directory("notes")
        .expect("a directory is created")
        .file("todo.txt", 120_000, Fill::Noise(3))
        .expect("a file is written");
    let report = scan(tree_on_disk.path(), &ScanOptions::default()).expect("the tree scans");
    let index = Index::build(
        &report,
        IndexRoot::for_path(tree_on_disk.path()).expect("the root kind is known"),
        &Enrichment::default(),
    );
    let tree = DiagramTree::from_index(&index, index.root(), DEPTH);
    assert_eq!(tree.node(ROOT).size, index.node(index.root()).allocated);

    let palette = &Palette::LIGHT;
    let layout = SunburstLayout::compute(&tree, ROOT, SIDE, 1.0);
    let largest = layout
        .segments
        .iter()
        .filter(|segment| segment.ring == 0)
        .max_by(|left, right| (left.end_angle - left.start_angle).total_cmp(&(right.end_angle - right.start_angle)))
        .expect("the index yields a first ring");
    let mut atlas = GlyphAtlas::default();
    let painter = DiagramPainter {
        tree: &tree,
        palette,
        strings: &STRINGS,
        scale: 1.0,
        hovered: None,
        dim_progress: 0.0,
        center: None,
    };
    let scene = painter.sunburst(&layout, None, 1.0, &mut atlas);
    let frame = renderer().render_offscreen(&scene, &atlas).image;
    frame
        .write_png(&crate_path("../../target/snapshots/render/smoke-index.png"))
        .expect("the frame is written");
    let (angle, radius) = largest.middle();
    let probe = layout.point_at(angle, radius);
    let [blue, green, red, _] = frame.pixel(probe.x as usize, probe.y as usize);
    let expected = category_color(palette, tree.node(largest.node).category)
        .premultiplied()
        .map(|channel| (channel * 255.0).round() as u8);
    for (actual, wanted) in [red, green, blue].into_iter().zip(expected) {
        assert!(
            actual.abs_diff(wanted) <= CHANNEL_TOLERANCE,
            "the segment shows {:?}, the token is {expected:?}",
            [red, green, blue]
        );
    }
}
