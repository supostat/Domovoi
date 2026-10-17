use objc2::rc::Retained;
use objc2_app_kit::NSColor;

#[derive(Debug)]
pub struct ToggleStyle {
    pub on: Retained<NSColor>,
    pub on_mark: Retained<NSColor>,
    pub off: Retained<NSColor>,
    pub off_mark: Retained<NSColor>,
}
