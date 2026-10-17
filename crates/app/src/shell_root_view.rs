use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSResponder, NSView};
use objc2_foundation::NSRect;

#[derive(Default)]
pub(crate) struct ShellRootViewState {
    on_appearance_change: RefCell<Option<Rc<dyn Fn()>>>,
}

impl std::fmt::Debug for ShellRootViewState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ShellRootViewState").finish_non_exhaustive()
    }
}

define_class!(
    #[unsafe(super(NSView, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ShellRootViewState]
    #[derive(Debug)]
    pub(crate) struct ShellRootView;

    impl ShellRootView {
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            let notify = self.ivars().on_appearance_change.borrow().clone();
            if let Some(notify) = notify {
                notify();
            }
        }
    }
);

impl ShellRootView {
    pub(crate) fn new(marker: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(ShellRootViewState::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub(crate) fn on_appearance_change(&self, notify: Rc<dyn Fn()>) {
        *self.ivars().on_appearance_change.borrow_mut() = Some(notify);
    }

    pub(crate) fn show(&self, content: &NSView) {
        for child in &self.subviews() {
            child.removeFromSuperview();
        }
        content.setFrame(self.bounds());
        self.addSubview(content);
    }
}
