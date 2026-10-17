use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSTextDelegate, NSTextField, NSTextView, NSTextViewDelegate};
use objc2_foundation::{NSNotification, NSObject};

use crate::shell_action::ShellAction;
use crate::shell_actions::ShellActions;

#[derive(Debug)]
pub(crate) struct ChatInputState {
    actions: Retained<ShellActions>,
    input: Retained<NSTextView>,
    placeholder: Retained<NSTextField>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ChatInputState]
    #[derive(Debug)]
    pub(crate) struct ChatInputDelegate;

    unsafe impl NSObjectProtocol for ChatInputDelegate {}

    unsafe impl NSTextDelegate for ChatInputDelegate {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            let state = self.ivars();
            state.placeholder.setHidden(state.input.string().length() > 0);
        }
    }

    unsafe impl NSTextViewDelegate for ChatInputDelegate {
        #[unsafe(method(textView:doCommandBySelector:))]
        fn do_command(&self, _view: &NSTextView, command: Sel) -> bool {
            let submits = command == sel!(insertNewline:);
            if submits {
                let actions = self.ivars().actions.clone();
                actions.dispatch(ShellAction::AskAssistant);
            }
            submits
        }
    }
);

impl ChatInputDelegate {
    pub(crate) fn new(
        marker: MainThreadMarker,
        actions: Retained<ShellActions>,
        input: Retained<NSTextView>,
        placeholder: Retained<NSTextField>,
    ) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(ChatInputState {
            actions,
            input,
            placeholder,
        });
        unsafe { msg_send![super(this), init] }
    }
}
