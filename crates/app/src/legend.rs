use domovoi_render::category_color;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{NSLineBreakMode, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::fill_view::{FillStyle, FillView};
use crate::label::{label, measure_label};
use crate::legend_entry::LegendEntry;
use crate::shell_palette::ShellPalette;
use crate::ui_font::{interface_font, mono_font};

const DOT: f64 = 8.0;
const CHIP_HEIGHT: f64 = 24.0;
const CHIP_PADDING: f64 = 9.0;
const INNER_GAP: f64 = 6.0;
const CHIP_GAP: f64 = 8.0;

pub(crate) fn legend(
    marker: MainThreadMarker,
    entries: &[LegendEntry],
    palette: &ShellPalette,
    width_limit: f64,
) -> Retained<NSView> {
    let chips: Vec<Retained<FillView>> = entries.iter().map(|entry| chip(marker, entry, palette)).collect();
    let mut rows: Vec<Vec<&FillView>> = vec![Vec::new()];
    let mut row_width = 0.0;
    for chip in &chips {
        let width = chip.frame().size.width;
        let current = rows.last_mut().expect("rows start with one row");
        if !current.is_empty() && row_width + CHIP_GAP + width > width_limit {
            rows.push(vec![chip]);
            row_width = width;
        } else {
            row_width += if current.is_empty() { width } else { CHIP_GAP + width };
            current.push(chip);
        }
    }
    let height = rows.len() as f64 * (CHIP_HEIGHT + CHIP_GAP) - CHIP_GAP;
    let container = NSView::initWithFrame(
        NSView::alloc(marker),
        NSRect::new(NSPoint::ZERO, NSSize::new(width_limit, height)),
    );
    for (position, row) in rows.iter().enumerate() {
        let y = height - (position + 1) as f64 * CHIP_HEIGHT - position as f64 * CHIP_GAP;
        let mut x = 0.0;
        for chip in row {
            chip.setFrameOrigin(NSPoint::new(x, y));
            x += chip.frame().size.width + CHIP_GAP;
            container.addSubview(chip);
        }
    }
    container
}

fn chip(marker: MainThreadMarker, entry: &LegendEntry, palette: &ShellPalette) -> Retained<FillView> {
    {
        let title = label(
            marker,
            &entry.title,
            &interface_font(12.0, 500.0),
            &palette.tokens.text.ns_color(),
            NSLineBreakMode::ByClipping,
        );
        let size = measure_label(
            marker,
            &entry.size_text,
            &mono_font(11.5, 400.0),
            &palette.tokens.dim.ns_color(),
        );
        let title_size = title.fittingSize();
        let size_size = size.fittingSize();
        let width = CHIP_PADDING + DOT + INNER_GAP + title_size.width + INNER_GAP + size_size.width + CHIP_PADDING;
        let chip = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(width, CHIP_HEIGHT)),
            FillStyle {
                fill: Some(palette.tokens.surface.ns_color()),
                border: Some(palette.line()),
                radius: CHIP_HEIGHT / 2.0,
            },
        );
        let dot = FillView::new(
            marker,
            NSRect::new(
                NSPoint::new(CHIP_PADDING, (CHIP_HEIGHT - DOT) / 2.0),
                NSSize::new(DOT, DOT),
            ),
            FillStyle {
                fill: Some(category_color(palette.tokens, entry.category).ns_color()),
                border: None,
                radius: DOT / 2.0,
            },
        );
        title.setFrame(NSRect::new(
            NSPoint::new(CHIP_PADDING + DOT + INNER_GAP, (CHIP_HEIGHT - title_size.height) / 2.0),
            title_size,
        ));
        size.setFrame(NSRect::new(
            NSPoint::new(
                width - CHIP_PADDING - size_size.width,
                (CHIP_HEIGHT - size_size.height) / 2.0,
            ),
            size_size,
        ));
        for child in [&*dot as &NSView, &title, &size] {
            chip.addSubview(child);
        }
        chip
    }
}
