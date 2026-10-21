use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSAccessibility, NSColor, NSFont, NSLayoutConstraintOrientation, NSLineBreakMode, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

const REQUIRED: f32 = 1000.0;

pub(crate) fn label(
    marker: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
    line_break: NSLineBreakMode,
) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), marker);
    field.setFont(Some(font));
    field.setTextColor(Some(color));
    field.setLineBreakMode(line_break);
    field.setDrawsBackground(false);
    if line_break != NSLineBreakMode::ByWordWrapping {
        field.setMaximumNumberOfLines(1);
    }
    field.sizeToFit();
    field
}

pub(crate) fn name_label(
    marker: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
    line_break: NSLineBreakMode,
) -> Retained<NSTextField> {
    let field = label(marker, text, font, color, line_break);
    let full = NSString::from_str(text);
    field.setToolTip(Some(&full));
    field.setAccessibilityLabel(Some(&full));
    field
}

pub(crate) fn measure_label(
    marker: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
) -> Retained<NSTextField> {
    let field = label(marker, text, font, color, NSLineBreakMode::ByClipping);
    field.setContentCompressionResistancePriority_forOrientation(REQUIRED, NSLayoutConstraintOrientation::Horizontal);
    field
}

pub(crate) fn retext(field: &NSTextField, text: &str) {
    field.setStringValue(&NSString::from_str(text));
    field.sizeToFit();
}

pub(crate) fn place(field: &NSTextField, origin: NSPoint, width: f64) {
    let height = field.fittingSize().height;
    field.setFrame(NSRect::new(origin, NSSize::new(width, height)));
}
