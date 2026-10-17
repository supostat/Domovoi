use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBezierPath, NSButton, NSButtonType, NSControl, NSControlStateValueOff, NSControlStateValueOn,
    NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::toggle_style::ToggleStyle;

pub(crate) const SIZE: NSSize = NSSize::new(34.0, 20.0);
const KNOB: f64 = 14.0;
const INSET: f64 = 3.0;

define_class!(
    #[unsafe(super(NSButton, NSControl, NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ToggleStyle]
    #[derive(Debug)]
    pub struct ToggleSwitch;

    impl ToggleSwitch {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let style = self.ivars();
            let on = self.state() == NSControlStateValueOn;
            let bounds = self.bounds();
            let track = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                bounds,
                bounds.size.height / 2.0,
                bounds.size.height / 2.0,
            );
            if on { &style.on } else { &style.off }.setFill();
            track.fill();
            let knob_x = if on { bounds.size.width - INSET - KNOB } else { INSET };
            let knob = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(knob_x, (bounds.size.height - KNOB) / 2.0),
                NSSize::new(KNOB, KNOB),
            ));
            if on { &style.on_mark } else { &style.off_mark }.setFill();
            knob.fill();
        }
    }
);

impl ToggleSwitch {
    pub(crate) fn new(
        marker: MainThreadMarker,
        title: &str,
        on: bool,
        palette: &ShellPalette,
        actions: &ShellActions,
        selector: Sel,
    ) -> Retained<Self> {
        let tokens = palette.tokens;
        let this = Self::alloc(marker).set_ivars(ToggleStyle {
            on: tokens.wax.ns_color(),
            on_mark: tokens.wax_ink.ns_color(),
            off: tokens.line.ns_color(),
            off_mark: tokens.text.ns_color(),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, SIZE)] };
        this.setButtonType(NSButtonType::Switch);
        this.setTitle(&NSString::new());
        this.setAccessibilityLabel(Some(&NSString::from_str(title)));
        this.setState(if on {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        unsafe {
            this.setTarget(Some(actions.target()));
            this.setAction(Some(selector));
        }
        this
    }
}
