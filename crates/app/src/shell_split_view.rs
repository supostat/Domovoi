use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBezierPath, NSColor, NSResponder, NSSplitView, NSSplitViewDividerStyle, NSView};
use objc2_foundation::NSRect;

#[derive(Debug)]
pub(crate) struct ShellSplitViewState {
    divider: RefCell<Retained<NSColor>>,
    backing: Retained<NSColor>,
}

define_class!(
    #[unsafe(super(NSSplitView, NSView, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ShellSplitViewState]
    #[derive(Debug)]
    pub(crate) struct ShellSplitView;

    impl ShellSplitView {
        #[unsafe(method_id(dividerColor))]
        fn divider_color(&self) -> Retained<NSColor> {
            self.ivars().divider.borrow().clone()
        }

        #[unsafe(method(drawDividerInRect:))]
        fn draw_divider_in_rect(&self, rect: NSRect) {
            self.ivars().backing.setFill();
            NSBezierPath::fillRect(rect);
            self.ivars().divider.borrow().setFill();
            NSBezierPath::fillRect(rect);
        }
    }
);

impl ShellSplitView {
    pub(crate) fn new(
        marker: MainThreadMarker,
        frame: NSRect,
        divider: Retained<NSColor>,
        backing: Retained<NSColor>,
    ) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(ShellSplitViewState {
            divider: RefCell::new(divider),
            backing,
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setVertical(true);
        this.setDividerStyle(NSSplitViewDividerStyle::Thin);
        this
    }
}
