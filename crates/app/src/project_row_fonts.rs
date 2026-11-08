use objc2::rc::Retained;
use objc2_app_kit::NSFont;

use crate::ui_font::{interface_font, mono_font};

#[derive(Debug)]
pub(crate) struct ProjectRowFonts {
    pub(crate) heading: Retained<NSFont>,
    pub(crate) summary: Retained<NSFont>,
    pub(crate) name: Retained<NSFont>,
    pub(crate) path: Retained<NSFont>,
    pub(crate) artifacts: Retained<NSFont>,
    pub(crate) age: Retained<NSFont>,
    pub(crate) size: Retained<NSFont>,
}

impl ProjectRowFonts {
    pub(crate) fn new() -> Self {
        Self {
            heading: interface_font(11.5, 600.0),
            summary: mono_font(11.5, 500.0),
            name: interface_font(13.5, 600.0),
            path: mono_font(11.0, 400.0),
            artifacts: mono_font(10.5, 500.0),
            age: mono_font(11.5, 400.0),
            size: mono_font(13.0, 600.0),
        }
    }
}
