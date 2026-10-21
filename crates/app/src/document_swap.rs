use objc2_app_kit::{NSScrollView, NSView};
use objc2_foundation::NSPoint;

const BOTTOM_TOLERANCE: f64 = 1.0;

pub(crate) fn swap_document_keeping_offset(scroll: &NSScrollView, document: &NSView) {
    swap_document(scroll, document, false);
}

pub(crate) fn swap_document_following_bottom(scroll: &NSScrollView, document: &NSView) {
    let at_bottom = shows_bottom(scroll);
    swap_document(scroll, document, at_bottom);
}

fn swap_document(scroll: &NSScrollView, document: &NSView, to_bottom: bool) {
    let clip = scroll.contentView();
    let visible = clip.bounds();
    scroll.setDocumentView(Some(document));
    let lowest = (document.frame().size.height - visible.size.height).max(0.0);
    let top = if to_bottom {
        lowest
    } else {
        visible.origin.y.min(lowest)
    };
    clip.scrollToPoint(NSPoint::new(0.0, top));
    scroll.reflectScrolledClipView(&clip);
}

fn shows_bottom(scroll: &NSScrollView) -> bool {
    let Some(document) = scroll.documentView() else {
        return true;
    };
    let visible = scroll.contentView().bounds();
    visible.origin.y + visible.size.height >= document.frame().size.height - BOTTOM_TOLERANCE
}
