use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::fill_view::{FillStyle, FillView};
use crate::shell_layout::from_top;

pub(crate) type Parts = Vec<(Retained<NSView>, NSPoint, NSSize)>;

pub(crate) fn content_extent(parts: &Parts) -> NSSize {
    parts.iter().fold(NSSize::new(0.0, 0.0), |extent, (_, origin, size)| {
        NSSize::new(
            extent.width.max(origin.x + size.width),
            extent.height.max(origin.y + size.height),
        )
    })
}

pub(crate) fn assemble(marker: MainThreadMarker, parts: Parts, padding: NSSize, style: FillStyle) -> Retained<NSView> {
    let content = content_extent(&parts);
    let size = NSSize::new(
        (content.width + 2.0 * padding.width).ceil(),
        (content.height + 2.0 * padding.height).ceil(),
    );
    let container = FillView::new(marker, NSRect::new(NSPoint::ZERO, size), style);
    for (view, origin, part_size) in parts {
        view.setFrame(from_top(
            size,
            padding.width + origin.x,
            padding.height + origin.y,
            part_size,
        ));
        container.addSubview(&view);
    }
    container.into_super()
}
