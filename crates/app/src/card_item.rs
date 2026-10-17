use objc2::{MainThreadMarker, MainThreadOnly, define_class};
use objc2_app_kit::{NSCollectionViewItem, NSResponder, NSView, NSViewController};

define_class!(
    #[unsafe(super(NSCollectionViewItem, NSViewController, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug)]
    pub struct CardItem;

    impl CardItem {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            self.setView(&NSView::new(MainThreadMarker::from(self)));
        }
    }
);
