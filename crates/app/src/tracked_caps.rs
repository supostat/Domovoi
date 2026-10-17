use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSColor, NSFontAttributeName, NSForegroundColorAttributeName, NSKernAttributeName};
use objc2_foundation::{NSAttributedString, NSDictionary, NSNumber, NSString};

use crate::ui_font::interface_font;

pub(crate) fn tracked_caps(text: &str, size: f64, tracking: f64, color: &NSColor) -> Retained<NSAttributedString> {
    let font = interface_font(size, 600.0);
    let kern = NSNumber::new_f64(size * tracking);
    let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName, NSKernAttributeName] };
    let values: [&AnyObject; 3] = [font.as_ref(), color.as_ref(), kern.as_ref()];
    let attributes = NSDictionary::from_slices(&keys, &values);
    unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(&text.to_uppercase()),
            Some(&attributes),
        )
    }
}
