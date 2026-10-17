mod support;

use std::env;
use std::path::PathBuf;

use domovoi_render::{
    BgraImage, DiagramPainter, DiagramRenderer, DiagramTree, FontFamily, GlyphAtlas, ROOT, Scene, ShapedLine,
    SunburstLayout, TextStyle, TreemapLayout,
};
use domovoi_theme::Palette;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGContext;
use objc2_core_text::CTLine;
use support::{STRINGS, book_v, crate_path, fonts, renderer};

const SUNBURST_SIZE: f32 = 460.0;
const TREEMAP_WIDTH: f32 = 820.0;
const TREEMAP_HEIGHT: f32 = 480.0;
const SCALES: [f32; 2] = [1.0, 2.0];
const CHANNEL_TOLERANCE: u8 = 24;
const MISMATCH_SHARE: f64 = 0.005;
const TEXT_MEAN_DIFFERENCE: f64 = 4.0;
const EXACT_VARIABLE: &str = "DOMOVOI_GOLDEN_EXACT";
const UPDATE_VARIABLE: &str = "DOMOVOI_GOLDEN_UPDATE";
const SAMPLE_TEXT: &str = "Xcode · DerivedData 38,2 ГБ";
const TEXT_PADDING: f32 = 8.0;
const TEXT_HEIGHT: f32 = 24.0;
const TEXT_BASELINE: f32 = 17.0;

fn themes() -> [(&'static str, &'static Palette); 2] {
    [("dark", &Palette::DARK), ("light", &Palette::LIGHT)]
}

fn painter<'tree>(palette: &'static Palette, scale: f32, tree: &'tree DiagramTree) -> DiagramPainter<'tree> {
    DiagramPainter {
        tree,
        palette,
        strings: &STRINGS,
        scale,
        hovered: None,
        dim_progress: 0.0,
        center: None,
    }
}

fn render(renderer: &mut DiagramRenderer, mode: &str, palette: &'static Palette, scale: f32) -> BgraImage {
    let tree = book_v();
    let mut atlas = GlyphAtlas::default();
    let painter = painter(palette, scale, &tree);
    let scene = if mode == "sunburst" {
        painter.sunburst(
            &SunburstLayout::compute(&tree, ROOT, SUNBURST_SIZE, 1.0),
            None,
            1.0,
            &mut atlas,
        )
    } else {
        painter.treemap(
            &TreemapLayout::compute(&tree, ROOT, TREEMAP_WIDTH, TREEMAP_HEIGHT),
            &mut atlas,
        )
    };
    renderer.render_offscreen(&scene, &atlas).image
}

fn mismatch_share(actual: &BgraImage, expected: &BgraImage) -> f64 {
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height),
        "the frame size changed"
    );
    let different = actual
        .pixels
        .chunks_exact(4)
        .zip(expected.pixels.chunks_exact(4))
        .filter(|(left, right)| {
            left.iter()
                .zip(right.iter())
                .any(|(a, b)| a.abs_diff(*b) > CHANNEL_TOLERANCE)
        })
        .count();
    different as f64 / (actual.width * actual.height) as f64
}

#[test]
fn book_v_renders_like_the_golden_frames_at_both_scales() {
    fonts();
    let mut renderer = renderer();
    let exact = env::var_os(EXACT_VARIABLE).is_some();
    let update = env::var_os(UPDATE_VARIABLE).is_some();
    for mode in ["sunburst", "treemap"] {
        for (theme, palette) in themes() {
            for scale in SCALES {
                let name = format!("{mode}-{theme}@{scale}x.png");
                let image = render(&mut renderer, mode, palette, scale);
                image
                    .write_png(&crate_path("../../target/snapshots/render").join(&name))
                    .expect("the snapshot is written");
                let golden: PathBuf = crate_path("tests/golden").join(&name);
                if update {
                    image.write_png(&golden).expect("the golden frame is written");
                    continue;
                }
                let expected = BgraImage::read_png(&golden)
                    .unwrap_or_else(|error| panic!("{name}: {error}; run with {UPDATE_VARIABLE}=1 to create it"));
                if exact {
                    assert!(image == expected, "{name} differs from the golden frame");
                } else {
                    let share = mismatch_share(&image, &expected);
                    assert!(
                        share <= MISMATCH_SHARE,
                        "{name}: {:.3} % of pixels differ",
                        share * 100.0
                    );
                }
            }
        }
    }
}

#[test]
fn atlas_text_matches_core_text_line_drawing() {
    fonts();
    let mut renderer = renderer();
    let tree = book_v();
    let style = TextStyle {
        family: FontFamily::Interface,
        weight: 600.0,
        size: 11.5,
    };
    for (_, palette) in themes() {
        for scale in SCALES {
            let line = ShapedLine::shape(SAMPLE_TEXT, style);
            let width = (line.width() + 2.0 * TEXT_PADDING).ceil();
            let mut atlas = GlyphAtlas::default();
            let mut scene = Scene {
                width,
                height: TEXT_HEIGHT,
                scale,
                progress: 1.0,
                clear: palette.surface.premultiplied(),
                ..Scene::default()
            };
            painter(palette, scale, &tree).paint_line(
                &mut scene,
                &mut atlas,
                &line,
                TEXT_PADDING,
                TEXT_BASELINE,
                palette.text.premultiplied(),
            );
            let metal = renderer.render_offscreen(&scene, &atlas).image;
            let reference = core_text_reference(line.line(), palette, scale, width);
            let difference: f64 = metal
                .pixels
                .iter()
                .zip(&reference.pixels)
                .map(|(a, b)| f64::from(a.abs_diff(*b)))
                .sum::<f64>()
                / metal.pixels.len() as f64;
            assert!(
                difference <= TEXT_MEAN_DIFFERENCE,
                "the atlas text differs from CTLineDraw by {difference:.2} at {scale}x"
            );
        }
    }
}

fn core_text_reference(line: &CTLine, palette: &Palette, scale: f32, width: f32) -> BgraImage {
    let pixel_width = (width * scale).round() as usize;
    let pixel_height = (TEXT_HEIGHT * scale).round() as usize;
    BgraImage::draw(pixel_width, pixel_height, |context| {
        let target = Some(context);
        let [red, green, blue, alpha] = palette.surface.straight().map(f64::from);
        CGContext::set_rgb_fill_color(target, red, green, blue, alpha);
        CGContext::fill_rect(
            target,
            CGRect::new(CGPoint::ZERO, CGSize::new(pixel_width as f64, pixel_height as f64)),
        );
        CGContext::scale_ctm(target, f64::from(scale), f64::from(scale));
        CGContext::set_allows_font_smoothing(target, true);
        CGContext::set_should_smooth_fonts(target, true);
        CGContext::set_allows_font_subpixel_positioning(target, true);
        CGContext::set_should_subpixel_position_fonts(target, true);
        CGContext::set_allows_font_subpixel_quantization(target, false);
        CGContext::set_should_subpixel_quantize_fonts(target, false);
        let [red, green, blue, alpha] = palette.text.straight().map(f64::from);
        CGContext::set_rgb_fill_color(target, red, green, blue, alpha);
        CGContext::set_text_position(target, f64::from(TEXT_PADDING), f64::from(TEXT_HEIGHT - TEXT_BASELINE));
        unsafe { line.draw(context) };
    })
}
