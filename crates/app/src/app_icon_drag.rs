use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSControl, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource,
    NSEvent, NSImage, NSImageScaling, NSImageView, NSResponder, NSView,
};
use objc2_foundation::{NSArray, NSBundle, NSRect, NSURL};

#[derive(Debug)]
pub(crate) struct AppIconDragState {
    bundle: Retained<NSURL>,
    image: Retained<NSImage>,
}

define_class!(
    #[unsafe(super(NSImageView, NSControl, NSView, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppIconDragState]
    #[derive(Debug)]
    pub(crate) struct AppIconView;

    unsafe impl NSObjectProtocol for AppIconView {}

    impl AppIconView {
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let state = self.ivars();
            let writer: &ProtocolObject<dyn objc2_app_kit::NSPasteboardWriting> = ProtocolObject::from_ref(&*state.bundle);
            let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), writer);
            unsafe { item.setDraggingFrame_contents(self.bounds(), Some(&state.image)) };
            self.beginDraggingSessionWithItems_event_source(
                &NSArray::from_retained_slice(&[item]),
                event,
                ProtocolObject::from_ref(self),
            );
        }
    }

    unsafe impl NSDraggingSource for AppIconView {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(&self, _session: &NSDraggingSession, _context: NSDraggingContext) -> NSDragOperation {
            NSDragOperation::Copy
        }
    }
);

impl AppIconView {
    pub(crate) fn in_bundle(marker: MainThreadMarker, frame: NSRect) -> Option<Retained<Self>> {
        let bundle = NSBundle::mainBundle();
        bundle.bundleIdentifier()?;
        let url = bundle.bundleURL();
        let image = NSApplication::sharedApplication(marker).applicationIconImage()?;
        let this = Self::alloc(marker).set_ivars(AppIconDragState {
            bundle: url,
            image: image.clone(),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setImage(Some(&image));
        this.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        Some(this)
    }
}
