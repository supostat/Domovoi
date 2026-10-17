use objc2_foundation::{NSPoint, NSRect, NSSize};

pub const WINDOW: NSSize = NSSize::new(1100.0, 700.0);
pub const NARROW_WINDOW: NSSize = NSSize::new(560.0, 600.0);
pub const SIDEBAR_WIDTH: f64 = 230.0;
pub(crate) const PANE_PADDING: f64 = 24.0;

pub(crate) fn from_top(container: NSSize, x: f64, top: f64, size: NSSize) -> NSRect {
    NSRect::new(NSPoint::new(x, container.height - top - size.height), size)
}
