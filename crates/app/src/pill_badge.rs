use domovoi_theme::Rgba;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSAccessibility, NSLineBreakMode};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::fill_view::{FillStyle, FillView};
use crate::label::label;
use crate::ui_font::interface_font;

const HEIGHT: f64 = 21.0;
const PADDING: f64 = 9.0;
const DOT: f64 = 6.0;
const GAP: f64 = 6.0;

pub(crate) fn pill_badge(marker: MainThreadMarker, title: &str, ink: Rgba, soft: Rgba) -> Retained<FillView> {
    let text = label(
        marker,
        title,
        &interface_font(11.0, 600.0),
        &ink.ns_color(),
        NSLineBreakMode::ByClipping,
    );
    let text_size = text.fittingSize();
    let width = PADDING + DOT + GAP + text_size.width + PADDING;
    let badge = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width.ceil(), HEIGHT)),
        FillStyle {
            fill: Some(soft.ns_color()),
            border: None,
            radius: HEIGHT / 2.0,
            dashed: false,
        },
    );
    let dot = FillView::new(
        marker,
        NSRect::new(NSPoint::new(PADDING, (HEIGHT - DOT) / 2.0), NSSize::new(DOT, DOT)),
        FillStyle {
            fill: Some(ink.ns_color()),
            border: None,
            radius: DOT / 2.0,
            dashed: false,
        },
    );
    text.setFrameOrigin(NSPoint::new(PADDING + DOT + GAP, (HEIGHT - text_size.height) / 2.0));
    badge.addSubview(&dot);
    badge.addSubview(&text);
    badge.setAccessibilityLabel(Some(&NSString::from_str(title)));
    badge
}
