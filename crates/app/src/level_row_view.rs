use std::cell::Cell;

use domovoi_render::category_color;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSAccessibility, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::fill_view::{FillStyle, FillView};
use crate::label::{measure_label, name_label};
use crate::level_row::LevelRow;
use crate::shell_palette::ShellPalette;
use crate::ui_font::{interface_font, mono_font};

const PADDING_X: f64 = 10.0;
const PADDING_Y: f64 = 9.0;
const DOT: f64 = 10.0;
const DOT_RADIUS: f64 = 3.0;
const DOT_COLUMN: f64 = 12.0;
const COLUMN_GAP: f64 = 12.0;
const ROW_GAP: f64 = 6.0;
const BAR_HEIGHT: f64 = 5.0;
const BAR_RADIUS: f64 = 3.0;

#[derive(Debug)]
pub struct LevelRowView {
    pub name: Retained<NSTextField>,
    pub size: Retained<NSTextField>,
    dot: Retained<FillView>,
    track: Retained<FillView>,
    bar: Retained<FillView>,
    share: Cell<f64>,
}

impl LevelRowView {
    pub fn build(marker: MainThreadMarker, container: &NSView, row: &LevelRow, palette: &ShellPalette) -> Self {
        let tokens = palette.tokens;
        let pill = |radius| FillStyle {
            fill: None,
            border: None,
            radius,
            dashed: false,
        };
        let parts = Self {
            name: name_label(
                marker,
                &row.name,
                &interface_font(13.0, 500.0),
                &tokens.text.ns_color(),
                NSLineBreakMode::ByTruncatingTail,
            ),
            size: measure_label(marker, &row.size_text, &mono_font(12.0, 500.0), &tokens.mute.ns_color()),
            dot: FillView::new(marker, NSRect::ZERO, pill(DOT_RADIUS)),
            track: FillView::new(marker, NSRect::ZERO, pill(BAR_RADIUS)),
            bar: FillView::new(marker, NSRect::ZERO, pill(BAR_RADIUS)),
            share: Cell::new(row.share),
        };
        for child in [
            &*parts.dot as &NSView,
            &parts.name,
            &parts.size,
            &parts.track,
            &parts.bar,
        ] {
            container.addSubview(child);
        }
        parts.update(row, palette);
        parts.layout(container.frame().size);
        parts
    }

    pub fn update(&self, row: &LevelRow, palette: &ShellPalette) {
        let color = category_color(palette.tokens, row.category).ns_color();
        let full = NSString::from_str(&row.name);
        self.name.setStringValue(&full);
        self.name.setToolTip(Some(&full));
        self.name.setAccessibilityLabel(Some(&full));
        self.size.setStringValue(&NSString::from_str(&row.size_text));
        self.dot.restyle(FillStyle {
            fill: Some(color.clone()),
            border: None,
            radius: DOT_RADIUS,
            dashed: false,
        });
        self.track.restyle(FillStyle {
            fill: Some(palette.line_soft()),
            border: None,
            radius: BAR_RADIUS,
            dashed: false,
        });
        self.bar.restyle(FillStyle {
            fill: Some(color),
            border: None,
            radius: BAR_RADIUS,
            dashed: false,
        });
        self.share.set(row.share.clamp(0.0, 1.0));
    }

    pub fn layout(&self, bounds: NSSize) {
        let content_left = PADDING_X + DOT_COLUMN + COLUMN_GAP;
        let content_width = (bounds.width - content_left - PADDING_X).max(0.0);
        let bar_y = PADDING_Y;
        self.track.setFrame(NSRect::new(
            NSPoint::new(content_left, bar_y),
            NSSize::new(content_width, BAR_HEIGHT),
        ));
        self.bar.setFrame(NSRect::new(
            NSPoint::new(content_left, bar_y),
            NSSize::new(content_width * self.share.get(), BAR_HEIGHT),
        ));
        let line_y = bar_y + BAR_HEIGHT + ROW_GAP;
        let size = self.size.fittingSize();
        let line_height = self.name.fittingSize().height.max(size.height);
        let size_x = bounds.width - PADDING_X - size.width;
        self.size.setFrame(NSRect::new(
            NSPoint::new(size_x, line_y + (line_height - size.height) / 2.0),
            size,
        ));
        self.name.setFrame(NSRect::new(
            NSPoint::new(content_left, line_y),
            NSSize::new((size_x - COLUMN_GAP - content_left).max(0.0), line_height),
        ));
        self.dot.setFrame(NSRect::new(
            NSPoint::new(PADDING_X, line_y + (line_height - DOT) / 2.0),
            NSSize::new(DOT, DOT),
        ));
    }
}
