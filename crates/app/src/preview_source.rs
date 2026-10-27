use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSInteger, NSObject, NSString, NSURL};
use objc2_quick_look_ui::{QLPreviewItem, QLPreviewPanel, QLPreviewPanelDataSource};

use crate::preview_targets::PreviewTargets;

#[derive(Debug, Default)]
pub(crate) struct PreviewSourceState {
    urls: RefCell<Vec<Retained<NSURL>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PreviewSourceState]
    #[derive(Debug)]
    pub(crate) struct PreviewSource;

    unsafe impl NSObjectProtocol for PreviewSource {}

    unsafe impl QLPreviewPanelDataSource for PreviewSource {
        #[unsafe(method(numberOfPreviewItemsInPreviewPanel:))]
        unsafe fn number_of_items(&self, _panel: Option<&QLPreviewPanel>) -> NSInteger {
            NSInteger::try_from(self.ivars().urls.borrow().len()).expect("a selection is small")
        }

        #[unsafe(method_id(previewPanel:previewItemAtIndex:))]
        unsafe fn item_at(
            &self,
            _panel: Option<&QLPreviewPanel>,
            index: NSInteger,
        ) -> Option<Retained<ProtocolObject<dyn QLPreviewItem>>> {
            let urls = self.ivars().urls.borrow();
            usize::try_from(index)
                .ok()
                .and_then(|position| urls.get(position).cloned())
                .map(ProtocolObject::from_retained)
        }
    }
);

impl PreviewSource {
    pub(crate) fn new(marker: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(PreviewSourceState::default());
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn set_targets(&self, targets: &PreviewTargets) {
        let urls = targets
            .paths
            .iter()
            .map(|path| NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy())))
            .collect();
        *self.ivars().urls.borrow_mut() = urls;
    }
}
