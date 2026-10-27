use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{NSControl, NSEvent, NSOutlineView, NSResponder, NSTableView, NSView};
use objc2_foundation::NSRect;

use crate::preview_key::toggles_preview;
use crate::shell_action::ShellAction;
use crate::shell_actions::ShellActions;

#[derive(Debug)]
pub(crate) struct PreviewOutlineState {
    actions: Retained<ShellActions>,
}

define_class!(
    #[unsafe(super(NSOutlineView, NSTableView, NSControl, NSView, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PreviewOutlineState]
    #[derive(Debug)]
    pub(crate) struct PreviewOutline;

    impl PreviewOutline {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if toggles_preview(event) {
                self.ivars().actions.dispatch(ShellAction::TogglePreview);
            } else {
                unsafe {
                    let _: () = msg_send![super(self), keyDown: event];
                }
            }
        }
    }
);

impl PreviewOutline {
    pub(crate) fn new(marker: MainThreadMarker, frame: NSRect, actions: &ShellActions) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(PreviewOutlineState {
            actions: actions.retain(),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}
