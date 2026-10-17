use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2_app_kit::NSColor;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellPalette {
    pub tokens: &'static Palette,
    pub high_contrast: bool,
}

impl ShellPalette {
    pub fn line(&self) -> Retained<NSColor> {
        if self.high_contrast {
            NSColor::separatorColor()
        } else {
            self.tokens.line.ns_color()
        }
    }

    pub fn line_soft(&self) -> Retained<NSColor> {
        if self.high_contrast {
            NSColor::separatorColor()
        } else {
            self.tokens.line_soft.ns_color()
        }
    }
}
