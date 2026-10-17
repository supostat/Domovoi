use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::fill_view::{FillStyle, FillView};
use crate::shell_layout::from_top;
use crate::shell_palette::ShellPalette;
use crate::window_frame::WindowFrame;

pub(crate) fn pane_head(
    marker: MainThreadMarker,
    pane: &NSView,
    height: f64,
    palette: &ShellPalette,
) -> Retained<FillView> {
    let size = WindowFrame::pane_size();
    let band = FillView::new(
        marker,
        from_top(size, 0.0, 0.0, NSSize::new(size.width, height)),
        FillStyle {
            fill: Some(palette.tokens.surface.ns_color()),
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    band.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin);
    let rule = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(size.width, 1.0)),
        FillStyle {
            fill: Some(palette.line_soft()),
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    rule.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    band.addSubview(&rule);
    pane.addSubview(&band);
    band
}
