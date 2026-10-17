use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBezierPath, NSColor, NSView};
use objc2_foundation::NSRect;

#[derive(Debug)]
pub struct FillStyle {
    pub fill: Option<Retained<NSColor>>,
    pub border: Option<Retained<NSColor>>,
    pub radius: f64,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<FillStyle>]
    #[derive(Debug)]
    pub struct FillView;

    impl FillView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let style = self.ivars().borrow();
            let bounds = self.bounds();
            let inset = if style.border.is_some() { 0.5 } else { 0.0 };
            let rect = NSRect::new(
                objc2_foundation::NSPoint::new(bounds.origin.x + inset, bounds.origin.y + inset),
                objc2_foundation::NSSize::new(bounds.size.width - 2.0 * inset, bounds.size.height - 2.0 * inset),
            );
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, style.radius, style.radius);
            if let Some(fill) = &style.fill {
                fill.setFill();
                path.fill();
            }
            if let Some(border) = &style.border {
                border.setStroke();
                path.setLineWidth(1.0);
                path.stroke();
            }
        }
    }
);

impl FillView {
    pub fn new(marker: MainThreadMarker, frame: NSRect, style: FillStyle) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(RefCell::new(style));
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub fn restyle(&self, style: FillStyle) {
        *self.ivars().borrow_mut() = style;
        self.setNeedsDisplay(true);
    }
}
