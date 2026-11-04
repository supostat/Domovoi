use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBezierPath, NSButton, NSColor, NSControl, NSDragOperation,
    NSDraggingDestination, NSDraggingInfo, NSFontAttributeName, NSForegroundColorAttributeName, NSView,
};
use objc2_foundation::{NSArray, NSAttributedString, NSDictionary, NSPoint, NSRect, NSSize, NSString};

use crate::drop_hover::DropHover;
use crate::drop_kind::DropKind;
use crate::node_pasteboard::node_type;
use crate::shell_action::ShellAction;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::strings::TRAY;
use crate::tray_drop_look::TrayDropLook;
use crate::ui_font::interface_font;

pub(crate) const RING_MARGIN: f64 = 5.0;
const RING_GAP: f64 = 3.0;
const RING_WIDTH: f64 = 2.0;
const WAITING_RING_ALPHA: f64 = 0.55;
const RADIUS: f64 = 8.0;
const HORIZONTAL_PADDING: f64 = 16.0;
const TITLE_SIZE: f64 = 13.0;
const TITLE_WEIGHT: f64 = 600.0;

#[derive(Debug)]
pub struct TrayDropButtonState {
    palette: ShellPalette,
    actions: Retained<ShellActions>,
    rest_title: RefCell<String>,
    hover: RefCell<Option<DropHover>>,
    look: Cell<TrayDropLook>,
}

define_class!(
    #[unsafe(super(NSButton, NSControl, NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TrayDropButtonState]
    #[derive(Debug)]
    pub struct TrayDropButton;

    impl TrayDropButton {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw();
        }
    }

    unsafe impl NSObjectProtocol for TrayDropButton {}

    unsafe impl NSDraggingDestination for TrayDropButton {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, _sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            self.show_hover()
        }

        #[unsafe(method(draggingUpdated:))]
        fn dragging_updated(&self, _sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            self.operation()
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
            self.leave_hover();
        }

        #[unsafe(method(prepareForDragOperation:))]
        fn prepare_for_drag(&self, _sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            self.operation() == NSDragOperation::Copy
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag(&self, _sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            self.ivars().actions.dispatch(ShellAction::DropOnTray);
            true
        }
    }
);

impl TrayDropButton {
    pub fn new(marker: MainThreadMarker, palette: &ShellPalette, actions: &ShellActions) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(TrayDropButtonState {
            palette: *palette,
            actions: actions.retain(),
            rest_title: RefCell::new(TRAY.to_owned()),
            hover: RefCell::new(None),
            look: Cell::new(TrayDropLook::Rest),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setBordered(false);
        let target: &AnyObject = actions.target();
        unsafe {
            this.setTarget(Some(target));
            this.setAction(Some(sel!(openTray:)));
        }
        this.registerForDraggedTypes(&NSArray::from_retained_slice(&[node_type()]));
        this.show_title(TRAY);
        this
    }

    pub fn show_summary(&self, title: &str) {
        title.clone_into(&mut self.ivars().rest_title.borrow_mut());
        if self.ivars().hover.borrow().is_none() {
            self.show_title(title);
        }
    }

    pub fn arm(&self, hover: DropHover) {
        let state = self.ivars();
        state.look.set(TrayDropLook::Waiting);
        let rest = state.rest_title.borrow().clone();
        let width = self.width_for(&rest).max(self.width_for(&hover.title));
        state.hover.replace(Some(hover));
        self.setTitle(&NSString::from_str(&rest));
        self.resize(width);
    }

    pub fn disarm(&self) {
        let state = self.ivars();
        state.hover.replace(None);
        state.look.set(TrayDropLook::Rest);
        let rest = state.rest_title.borrow().clone();
        self.show_title(&rest);
    }

    pub fn show_hover(&self) -> NSDragOperation {
        let state = self.ivars();
        let Some(hover) = state.hover.borrow().clone() else {
            return NSDragOperation::None;
        };
        state.look.set(TrayDropLook::of(hover.kind));
        self.setTitle(&NSString::from_str(&hover.title));
        AsRef::<NSView>::as_ref(self).setNeedsDisplay(true);
        self.operation()
    }

    pub fn leave_hover(&self) {
        let state = self.ivars();
        if state.hover.borrow().is_none() {
            return;
        }
        state.look.set(TrayDropLook::Waiting);
        self.setTitle(&NSString::from_str(&state.rest_title.borrow()));
        AsRef::<NSView>::as_ref(self).setNeedsDisplay(true);
    }

    pub fn look(&self) -> TrayDropLook {
        self.ivars().look.get()
    }

    pub fn hover_kind(&self) -> Option<DropKind> {
        self.ivars().hover.borrow().as_ref().map(|hover| hover.kind)
    }

    pub fn shown_title(&self) -> String {
        self.title().to_string()
    }

    pub fn body_frame(&self) -> NSRect {
        let frame = self.frame();
        NSRect::new(
            NSPoint::new(frame.origin.x + RING_MARGIN, frame.origin.y + RING_MARGIN),
            NSSize::new(
                frame.size.width - 2.0 * RING_MARGIN,
                frame.size.height - 2.0 * RING_MARGIN,
            ),
        )
    }

    pub fn contains_screen_point(&self, point: NSPoint) -> bool {
        let Some(window) = self.window() else {
            return false;
        };
        let local = self.convertPoint_fromView(window.convertPointFromScreen(point), None);
        let bounds = self.bounds();
        local.x >= bounds.origin.x
            && local.y >= bounds.origin.y
            && local.x <= bounds.origin.x + bounds.size.width
            && local.y <= bounds.origin.y + bounds.size.height
    }

    pub(crate) fn set_body_height(&self, height: f64) {
        let frame = self.frame();
        self.setFrameSize(NSSize::new(frame.size.width, height + 2.0 * RING_MARGIN));
    }

    fn operation(&self) -> NSDragOperation {
        match self.hover_kind() {
            Some(DropKind::Add) => NSDragOperation::Copy,
            _ => NSDragOperation::None,
        }
    }

    fn show_title(&self, title: &str) {
        self.setTitle(&NSString::from_str(title));
        self.resize(self.width_for(title));
    }

    fn resize(&self, width: f64) {
        let frame = self.frame();
        let right = frame.origin.x + frame.size.width;
        self.setFrame(NSRect::new(
            NSPoint::new(right - width, frame.origin.y),
            NSSize::new(width, frame.size.height),
        ));
        AsRef::<NSView>::as_ref(self).setNeedsDisplay(true);
    }

    fn width_for(&self, title: &str) -> f64 {
        let text = attributed_title(title, &self.ivars().palette.tokens.wax_ink.ns_color());
        (text.size().width + 2.0 * HORIZONTAL_PADDING).ceil() + 2.0 * RING_MARGIN
    }

    fn draw(&self) {
        let tokens = self.ivars().palette.tokens;
        let look = self.look();
        let (fill, text) = match look {
            TrayDropLook::Rest | TrayDropLook::Waiting | TrayDropLook::Add => {
                (tokens.wax.ns_color(), tokens.wax_ink.ns_color())
            }
            TrayDropLook::Denied => (tokens.surface_2.ns_color(), tokens.risk.ns_color()),
            TrayDropLook::Already => (tokens.surface_2.ns_color(), tokens.mute.ns_color()),
        };
        let ring = match look {
            TrayDropLook::Rest => None,
            TrayDropLook::Waiting => Some(tokens.wax.ns_color().colorWithAlphaComponent(WAITING_RING_ALPHA)),
            TrayDropLook::Add => Some(tokens.wax.ns_color()),
            TrayDropLook::Denied => Some(tokens.risk.ns_color()),
            TrayDropLook::Already => Some(self.ivars().palette.line()),
        };
        let bounds = self.bounds();
        let body = NSRect::new(
            NSPoint::new(bounds.origin.x + RING_MARGIN, bounds.origin.y + RING_MARGIN),
            NSSize::new(
                bounds.size.width - 2.0 * RING_MARGIN,
                bounds.size.height - 2.0 * RING_MARGIN,
            ),
        );
        if let Some(ring) = ring {
            let outset = RING_GAP + RING_WIDTH / 2.0;
            let around = NSRect::new(
                NSPoint::new(body.origin.x - outset, body.origin.y - outset),
                NSSize::new(body.size.width + 2.0 * outset, body.size.height + 2.0 * outset),
            );
            let path =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(around, RADIUS + outset, RADIUS + outset);
            path.setLineWidth(RING_WIDTH);
            ring.setStroke();
            path.stroke();
        }
        fill.setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(body, RADIUS, RADIUS).fill();
        let title = attributed_title(&self.title().to_string(), &text);
        let size = title.size();
        title.drawAtPoint(NSPoint::new(
            body.origin.x + (body.size.width - size.width) / 2.0,
            body.origin.y + (body.size.height - size.height) / 2.0,
        ));
    }
}

fn attributed_title(title: &str, color: &NSColor) -> Retained<NSAttributedString> {
    let font = interface_font(TITLE_SIZE, TITLE_WEIGHT);
    let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [font.as_ref(), color.as_ref()];
    let attributes = NSDictionary::from_slices(&keys, &values);
    unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(title),
            Some(&attributes),
        )
    }
}
