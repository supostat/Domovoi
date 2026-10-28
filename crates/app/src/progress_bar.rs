use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::fill_view::{FillStyle, FillView};
use crate::shell_palette::ShellPalette;

pub(crate) const PROGRESS_HEIGHT: f64 = 8.0;

pub(crate) fn progress_bar(
    marker: MainThreadMarker,
    width: f64,
    share: f64,
    palette: &ShellPalette,
) -> Retained<NSView> {
    let track = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width, PROGRESS_HEIGHT)),
        FillStyle {
            fill: Some(palette.line_soft()),
            border: None,
            radius: PROGRESS_HEIGHT / 2.0,
            dashed: false,
        },
    );
    let fill = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new((width * share).round(), PROGRESS_HEIGHT)),
        FillStyle {
            fill: Some(palette.tokens.wax.ns_color()),
            border: None,
            radius: PROGRESS_HEIGHT / 2.0,
            dashed: false,
        },
    );
    track.addSubview(&fill);
    track.into_super()
}
