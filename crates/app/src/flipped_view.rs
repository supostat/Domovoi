use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::NSView;
use objc2_foundation::NSRect;

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug)]
    pub(crate) struct FlippedView;

    impl FlippedView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl FlippedView {
    pub(crate) fn new(marker: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}
