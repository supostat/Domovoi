use objc2_app_kit::NSEvent;

const SPACE: u16 = 49;

pub(crate) fn toggles_preview(event: &NSEvent) -> bool {
    event.keyCode() == SPACE
}
