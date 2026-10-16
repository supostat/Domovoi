use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{
    NSAppearanceName, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter};

pub(crate) fn aqua_appearance_name() -> &'static NSAppearanceName {
    unsafe { NSAppearanceNameAqua }
}

pub(crate) fn dark_aqua_appearance_name() -> &'static NSAppearanceName {
    unsafe { NSAppearanceNameDarkAqua }
}

pub(crate) fn observe_accessibility_display_options(
    center: &NSNotificationCenter,
    handler: impl Fn() + Send + Sync + 'static,
) -> Retained<ProtocolObject<dyn NSObjectProtocol>> {
    let block = RcBlock::new(move |_notification: NonNull<NSNotification>| handler());
    unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),
            None,
            None,
            &block,
        )
    }
}

pub(crate) fn remove_observer(center: &NSNotificationCenter, observer: &ProtocolObject<dyn NSObjectProtocol>) {
    unsafe { center.removeObserver(observer.as_ref()) }
}
