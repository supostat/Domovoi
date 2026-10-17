use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBezierPath, NSButton, NSButtonType, NSControl, NSControlStateValueOff, NSControlStateValueOn,
    NSLineCapStyle, NSLineJoinStyle, NSView,
};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString};

use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::toggle_style::ToggleStyle;

pub(crate) const SIDE: f64 = 16.0;
const RADIUS: f64 = 4.0;
const DISABLED_ALPHA: f64 = 0.4;
const MARK_WIDTH: f64 = 2.0;

define_class!(
    #[unsafe(super(NSButton, NSControl, NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ToggleStyle]
    #[derive(Debug)]
    pub struct CheckBox;

    impl CheckBox {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let style = self.ivars();
            let checked = self.state() == NSControlStateValueOn;
            let alpha = if self.isEnabled() { 1.0 } else { DISABLED_ALPHA };
            let bounds = self.bounds();
            let rect = NSRect::new(
                NSPoint::new(bounds.origin.x + 0.5, bounds.origin.y + 0.5),
                NSSize::new(bounds.size.width - 1.0, bounds.size.height - 1.0),
            );
            let box_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, RADIUS, RADIUS);
            if checked {
                style.on.colorWithAlphaComponent(alpha).setFill();
                box_path.fill();
                let mark = NSBezierPath::bezierPath();
                mark.moveToPoint(NSPoint::new(bounds.size.width * 0.25, bounds.size.height * 0.50));
                mark.lineToPoint(NSPoint::new(bounds.size.width * 0.43, bounds.size.height * 0.69));
                mark.lineToPoint(NSPoint::new(bounds.size.width * 0.76, bounds.size.height * 0.31));
                mark.setLineWidth(MARK_WIDTH);
                mark.setLineCapStyle(NSLineCapStyle::Round);
                mark.setLineJoinStyle(NSLineJoinStyle::Round);
                style.on_mark.colorWithAlphaComponent(alpha).setStroke();
                mark.stroke();
            } else {
                style.off.colorWithAlphaComponent(alpha).setFill();
                box_path.fill();
                style.off_mark.colorWithAlphaComponent(alpha).setStroke();
                box_path.setLineWidth(1.0);
                box_path.stroke();
            }
        }
    }
);

impl CheckBox {
    pub(crate) fn new(
        marker: MainThreadMarker,
        title: &str,
        checked: bool,
        enabled: bool,
        palette: &ShellPalette,
        action: (&ShellActions, Sel, NSInteger),
    ) -> Retained<Self> {
        let tokens = palette.tokens;
        let this = Self::alloc(marker).set_ivars(ToggleStyle {
            on: tokens.wax.ns_color(),
            on_mark: tokens.wax_ink.ns_color(),
            off: tokens.surface.ns_color(),
            off_mark: palette.line(),
        });
        let this: Retained<Self> =
            unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, NSSize::new(SIDE, SIDE))] };
        this.setButtonType(NSButtonType::Switch);
        this.setTitle(&NSString::new());
        this.setAccessibilityLabel(Some(&NSString::from_str(title)));
        this.setState(if checked {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        this.setEnabled(enabled);
        let (actions, selector, tag) = action;
        this.setTag(tag);
        unsafe {
            this.setTarget(Some(actions.target()));
            this.setAction(Some(selector));
        }
        this
    }
}
