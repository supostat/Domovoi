use std::path::PathBuf;
use std::time::Instant;

use domovoi_render::{BgraImage, DiagramRenderer, GlyphAtlas};
use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSGraphicsContext,
};
use objc2_core_graphics::CGContext;
use serde_json::json;

use crate::built_screen::BuiltScreen;
use crate::bundle_resources::diagram_metallib;
use crate::screen::Screen;
use crate::shell_actions::ShellActions;
use crate::shell_data::ShellData;
use crate::shell_palette::ShellPalette;
use crate::snapshot_options::SnapshotOptions;

const BYTES_PER_PIXEL: usize = 4;

pub fn write_snapshots(
    marker: MainThreadMarker,
    data: &ShellData,
    screens: &[Screen],
    options: &SnapshotOptions,
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(&options.directory).map_err(|error| format!("{}: {error}", options.directory.display()))?;
    let mut renderer = DiagramRenderer::system_default(&diagram_metallib()).map_err(|error| error.to_string())?;
    screens
        .iter()
        .map(|screen| {
            let options = SnapshotOptions {
                screen: *screen,
                ..options.clone()
            };
            write_snapshot(marker, data, &options, &mut renderer)
        })
        .collect()
}

pub fn write_snapshot(
    marker: MainThreadMarker,
    data: &ShellData,
    options: &SnapshotOptions,
    renderer: &mut DiagramRenderer,
) -> Result<PathBuf, String> {
    let tokens: &'static Palette = if options.dark { &Palette::DARK } else { &Palette::LIGHT };
    let palette = ShellPalette {
        tokens,
        high_contrast: options.high_contrast,
    };
    let actions = ShellActions::new(marker);
    let built = BuiltScreen::build(marker, options.screen, data, &palette, &actions);
    let root = built.root();
    let size = built.window();
    let appearance_name = unsafe {
        if options.dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        }
    };
    root.setAppearance(NSAppearance::appearanceNamed(appearance_name).as_deref());
    root.layoutSubtreeIfNeeded();
    let scale = options.scale;
    let width = (size.width * scale).round() as usize;
    let height = (size.height * scale).round() as usize;
    let mut image = BgraImage::draw(width, height, |context| {
        CGContext::scale_ctm(Some(context), scale, scale);
        let graphics = NSGraphicsContext::graphicsContextWithCGContext_flipped(context, false);
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&graphics));
        root.displayRectIgnoringOpacity_inContext(root.bounds(), &graphics);
        NSGraphicsContext::restoreGraphicsState_class();
    });
    if let Some((slot, state)) = built.diagram(data, &palette) {
        let mut atlas = GlyphAtlas::default();
        let mut scene = state.scene(Instant::now(), scale as f32, &mut atlas);
        scene.clear = tokens.bg.premultiplied();
        let frame = renderer.render_offscreen(&scene, &atlas).image;
        let left = (slot.origin.x * scale).round() as usize;
        let top = ((size.height - slot.origin.y - slot.size.height) * scale).round() as usize;
        paste(&mut image, &frame, left, top);
    }
    let stem = options.file_stem();
    let png = options.directory.join(format!("{stem}.png"));
    image.write_png(&png).map_err(|error| error.to_string())?;
    let manifest = json!({
        "window": [size.width, size.height],
        "scale": scale,
        "probes": built.probes(),
    });
    std::fs::write(
        options.directory.join(format!("{stem}.json")),
        serde_json::to_string_pretty(&manifest).expect("the manifest serialises"),
    )
    .map_err(|error| error.to_string())?;
    Ok(png)
}

fn paste(target: &mut BgraImage, source: &BgraImage, left: usize, top: usize) {
    for row in 0..source.height {
        let target_row = top + row;
        if target_row >= target.height {
            break;
        }
        let columns = source.width.min(target.width.saturating_sub(left));
        let from = row * source.width * BYTES_PER_PIXEL;
        let to = (target_row * target.width + left) * BYTES_PER_PIXEL;
        target.pixels[to..to + columns * BYTES_PER_PIXEL]
            .copy_from_slice(&source.pixels[from..from + columns * BYTES_PER_PIXEL]);
    }
}
