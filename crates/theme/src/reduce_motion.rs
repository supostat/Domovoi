use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSNotificationCenter;

use crate::ffi;

pub fn reduce_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

pub fn observe_reduce_motion(handler: impl Fn(bool) + Send + Sync + 'static) -> ReduceMotionObservation {
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let observer = ffi::observe_accessibility_display_options(&center, move || handler(reduce_motion()));
    ReduceMotionObservation { center, observer }
}

#[derive(Debug)]
pub struct ReduceMotionObservation {
    center: Retained<NSNotificationCenter>,
    observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for ReduceMotionObservation {
    fn drop(&mut self) {
        ffi::remove_observer(&self.center, &self.observer);
    }
}
