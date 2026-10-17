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
const DISABLED_ALPHA: f64 = 0.45;

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
            let alpha = if self.isEnabled() { 1.0 } else { DISABLED_ALPHA };
            let bounds = self.bounds();
            let rect = NSRect::new(
                NSPoint::new(bounds.origin.x + 0.5, bounds.origin.y + 0.5),
                NSSize::new(bounds.size.width - 1.0, bounds.size.height - 1.0),
            );
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, RADIUS, RADIUS);
            if let Some(fill) = &style.fill {
                fill.colorWithAlphaComponent(alpha).setFill();
                path.fill();
            }
            if let Some(border) = &style.border {
                border.colorWithAlphaComponent(alpha).setStroke();
                path.setLineWidth(1.0);
                path.stroke();
            }
            let title = self.attributed_title(alpha);
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
        let width = this.attributed_title(1.0).size().width + 2.0 * HORIZONTAL_PADDING;
        this.setFrameSize(NSSize::new(width.ceil(), height));
        this
    }

    pub fn on_click(&self, actions: &ShellActions, selector: Sel) {
        unsafe {
            self.setTarget(Some(actions.target()));
            self.setAction(Some(selector));
        }
    }

    pub fn retitle(&self, title: &str) {
        let right = self.frame().origin.x + self.frame().size.width;
        self.setTitle(&NSString::from_str(title));
        let width = (self.attributed_title(1.0).size().width + 2.0 * HORIZONTAL_PADDING).ceil();
        self.setFrame(NSRect::new(
            NSPoint::new(right - width, self.frame().origin.y),
            NSSize::new(width, self.frame().size.height),
        ));
        AsRef::<NSView>::as_ref(self).setNeedsDisplay(true);
    }

    fn attributed_title(&self, alpha: f64) -> Retained<NSAttributedString> {
        let style = self.ivars();
        let text = style.text.colorWithAlphaComponent(alpha);
        let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
        let values: [&AnyObject; 2] = [style.font.as_ref(), text.as_ref()];
        let attributes = NSDictionary::from_slices(&keys, &values);
        unsafe {
            NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &self.title(), Some(&attributes))
        }
    }
}
