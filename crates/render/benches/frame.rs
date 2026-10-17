use std::path::Path;
use std::time::Instant;

use domovoi_index::Category;
use domovoi_render::{
    BUILT_METALLIB, DiagramOutline, DiagramPainter, DiagramRenderer, DiagramStrings, DiagramTree, GlyphAtlas, ROOT,
    SunburstLayout, TreemapLayout, register_fonts,
};
use domovoi_theme::Palette;

const FIXTURES: [(&str, [usize; 3]); 2] = [("1 млн", [100, 100, 100]), ("5,65 млн", [150, 150, 251])];
const SUNBURST_SIDE: f32 = 600.0;
const TREEMAP_WIDTH: f32 = 1200.0;
const TREEMAP_HEIGHT: f32 = 760.0;
const SCALE: f32 = 2.0;
const STRINGS: DiagramStrings = DiagramStrings {
    category_names: [
        "apps", "dev", "media", "docs", "cache", "system", "hidden", "model", "home",
    ],
    used: "used",
    up: "up",
};

struct Sizes(u64);

impl Sizes {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % 10_000_000 + 4096
    }
}

fn outline(fanout: &[usize], sizes: &mut Sizes, label: usize) -> DiagramOutline {
    let category = Category::ALL[label % Category::ALL.len()];
    match fanout.split_first() {
        None => DiagramOutline {
            name: format!("file-{label}"),
            category,
            size: sizes.next(),
            children: Vec::new(),
        },
        Some((count, rest)) => DiagramOutline {
            name: format!("folder-{label}"),
            category,
            size: 0,
            children: (0..*count)
                .map(|child| outline(rest, sizes, label * 31 + child))
                .collect(),
        },
    }
}

fn main() {
    register_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts")).expect("the fonts register");
    let mut renderer = DiagramRenderer::system_default(Path::new(BUILT_METALLIB)).expect("the renderer starts");
    for (name, fanout) in FIXTURES {
        let tree = DiagramTree::from_outline(&outline(&fanout, &mut Sizes(7), 1));
        let started = Instant::now();
        let sunburst = SunburstLayout::compute(&tree, ROOT, SUNBURST_SIDE, 1.0);
        let sunburst_layout = started.elapsed();
        let started = Instant::now();
        let treemap = TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT);
        let treemap_layout = started.elapsed();
        let painter = DiagramPainter {
            tree: &tree,
            palette: &Palette::DARK,
            strings: &STRINGS,
            scale: SCALE,
            hovered: None,
            dim_progress: 0.0,
        };
        let mut atlas = GlyphAtlas::default();
        let sunburst_scene = painter.sunburst(&sunburst, None, 1.0, &mut atlas);
        let treemap_scene = painter.treemap(&treemap, &mut atlas);
        let sunburst_frame = renderer.render_offscreen(&sunburst_scene, &atlas);
        let treemap_frame = renderer.render_offscreen(&treemap_scene, &atlas);
        let labelled: f32 = treemap
            .tiles
            .iter()
            .filter(|tile| tile.label.is_some())
            .map(|tile| tile.rect.width * tile.rect.height)
            .sum();
        println!(
            "frame {name} узлов ({}): sunburst раскладка {sunburst_layout:?}, {} инстансов, GPU {:.3} мс; treemap раскладка {treemap_layout:?}, {} инстансов, GPU {:.3} мс, подписано {:.1} % площади",
            tree.len(),
            sunburst_scene.shapes.len() + sunburst_scene.glyphs.len(),
            sunburst_frame.gpu_seconds * 1000.0,
            treemap_scene.shapes.len() + treemap_scene.glyphs.len(),
            treemap_frame.gpu_seconds * 1000.0,
            labelled / (TREEMAP_WIDTH * TREEMAP_HEIGHT) * 100.0
        );
    }
}
