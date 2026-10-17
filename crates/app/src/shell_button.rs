use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBezierPath, NSButton, NSControl, NSFontAttributeName,
    NSForegroundColorAttributeName, NSView,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::shell_actions::ShellActions;

const RADIUS: f64 = 8.0;
const HORIZONTAL_PADDING: f64 = 16.0;

define_class!(
    #[unsafe(super(NSButton, NSControl, NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ButtonStyle]
    #[derive(Debug)]
    pub struct ShellButton;

    impl ShellButton {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let style = self.ivars();
            let bounds = self.bounds();
            let rect = NSRect::new(
                NSPoint::new(bounds.origin.x + 0.5, bounds.origin.y + 0.5),
                NSSize::new(bounds.size.width - 1.0, bounds.size.height - 1.0),
            );
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, RADIUS, RADIUS);
            if let Some(fill) = &style.fill {
                fill.setFill();
                path.fill();
            }
            if let Some(border) = &style.border {
                border.setStroke();
                path.setLineWidth(1.0);
                path.stroke();
            }
            let title = self.attributed_title();
            let size = title.size();
            let origin = NSPoint::new(
                (bounds.size.width - size.width) / 2.0,
                (bounds.size.height - size.height) / 2.0,
            );
            title.drawAtPoint(origin);
        }
    }
);

impl ShellButton {
    pub fn new(
        marker: MainThreadMarker,
        title: &str,
        origin: NSPoint,
        height: f64,
        style: ButtonStyle,
    ) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(style);
        let this: Retained<Self> =
            unsafe { msg_send![super(this), initWithFrame: NSRect::new(origin, NSSize::new(1.0, height))] };
        this.setTitle(&NSString::from_str(title));
        this.setBordered(false);
        let width = this.attributed_title().size().width + 2.0 * HORIZONTAL_PADDING;
        this.setFrameSize(NSSize::new(width.ceil(), height));
        this
    }

    pub fn on_click(&self, actions: &ShellActions, selector: Sel) {
        unsafe {
            self.setTarget(Some(actions.target()));
            self.setAction(Some(selector));
        }
    }

    fn attributed_title(&self) -> Retained<NSAttributedString> {
        let style = self.ivars();
        let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
        let values: [&AnyObject; 2] = [style.font.as_ref(), style.text.as_ref()];
        let attributes = NSDictionary::from_slices(&keys, &values);
        unsafe {
            NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &self.title(), Some(&attributes))
        }
    }
}
