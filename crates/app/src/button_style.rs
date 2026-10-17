use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSFont};

#[derive(Debug)]
pub struct ButtonStyle {
    pub fill: Option<Retained<NSColor>>,
    pub border: Option<Retained<NSColor>>,
    pub text: Retained<NSColor>,
    pub font: Retained<NSFont>,
}
