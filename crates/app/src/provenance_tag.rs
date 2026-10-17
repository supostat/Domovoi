use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::fill_view::{FillStyle, FillView};
use crate::label::measure_label;
use crate::provenance::Provenance;
use crate::shell_palette::ShellPalette;
use crate::ui_font::mono_font;

const HEIGHT: f64 = 21.0;
const PADDING: f64 = 8.0;
const RADIUS: f64 = 5.0;

pub(crate) fn provenance_tag(
    marker: MainThreadMarker,
    provenance: &Provenance,
    palette: &ShellPalette,
) -> Retained<FillView> {
    let tokens = palette.tokens;
    let hypothesis = matches!(provenance, Provenance::Hypothesis);
    let ink = if hypothesis { tokens.wax } else { tokens.mute };
    let text = measure_label(marker, &provenance.label(), &mono_font(11.0, 500.0), &ink.ns_color());
    let text_size = text.fittingSize();
    let tag = FillView::new(
        marker,
        NSRect::new(
            NSPoint::ZERO,
            NSSize::new((text_size.width + 2.0 * PADDING).ceil(), HEIGHT),
        ),
        FillStyle {
            fill: None,
            border: Some(if hypothesis {
                tokens.wax.ns_color()
            } else {
                palette.line()
            }),
            radius: RADIUS,
            dashed: hypothesis,
        },
    );
    text.setFrameOrigin(NSPoint::new(PADDING, (HEIGHT - text_size.height) / 2.0));
    tag.addSubview(&text);
    tag
}
