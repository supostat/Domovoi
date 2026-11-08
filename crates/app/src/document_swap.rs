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

pub(crate) fn scroll_to_offset_of(scroll: &NSScrollView, previous: &NSScrollView) {
    scroll_to(scroll, previous.contentView().bounds().origin.y);
}

fn swap_document(scroll: &NSScrollView, document: &NSView, to_bottom: bool) {
    let top = scroll.contentView().bounds().origin.y;
    scroll.setDocumentView(Some(document));
    scroll_to(scroll, if to_bottom { f64::INFINITY } else { top });
}

fn scroll_to(scroll: &NSScrollView, top: f64) {
    let clip = scroll.contentView();
    let document = scroll.documentView().expect("a scrolled view has a document");
    let lowest = (document.frame().size.height - clip.bounds().size.height).max(0.0);
    clip.scrollToPoint(NSPoint::new(0.0, top.min(lowest)));
    scroll.reflectScrolledClipView(&clip);
}

fn shows_bottom(scroll: &NSScrollView) -> bool {
    let Some(document) = scroll.documentView() else {
        return true;
    };
    let visible = scroll.contentView().bounds();
    visible.origin.y + visible.size.height >= document.frame().size.height - BOTTOM_TOLERANCE
}
