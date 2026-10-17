use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSTextAlignment, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::field_ornament::{FieldOrnament, OrnamentInk};
use crate::label::label;
use crate::shell_layout::from_top;
use crate::shell_palette::ShellPalette;
use crate::ui_font::interface_font;

const WIDTH: f64 = 420.0;
const GAP: f64 = 20.0;

pub(crate) fn empty_state(
    marker: MainThreadMarker,
    size: NSSize,
    text: &str,
    action: Option<&NSView>,
    palette: &ShellPalette,
) -> Retained<NSView> {
    let tokens = palette.tokens;
    let container = NSView::initWithFrame(NSView::alloc(marker), NSRect::new(NSPoint::ZERO, size));
    container.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    let message = label(
        marker,
        text,
        &interface_font(14.0, 400.0),
        &tokens.mute.ns_color(),
        NSLineBreakMode::ByWordWrapping,
    );
    message.setAlignment(NSTextAlignment::Center);
    message.setPreferredMaxLayoutWidth(WIDTH);
    let message_height = message.fittingSize().height;
    let action_height = action.map_or(0.0, |action| GAP + action.frame().size.height);
    let block = FieldOrnament::height() + GAP + message_height + action_height;
    let top = (size.height - block) / 2.0;
    let left = (size.width - WIDTH) / 2.0;
    let ornament = FieldOrnament::new(
        marker,
        from_top(size, left, top, NSSize::new(WIDTH, FieldOrnament::height())),
        OrnamentInk {
            outline: tokens.wax,
            fill: tokens.berry,
            center: tokens.text,
        },
    );
    message.setFrame(from_top(
        size,
        left,
        top + FieldOrnament::height() + GAP,
        NSSize::new(WIDTH, message_height),
    ));
    let centered = NSAutoresizingMaskOptions::ViewMinXMargin
        | NSAutoresizingMaskOptions::ViewMaxXMargin
        | NSAutoresizingMaskOptions::ViewMinYMargin
        | NSAutoresizingMaskOptions::ViewMaxYMargin;
    ornament.setAutoresizingMask(centered);
    message.setAutoresizingMask(centered);
    container.addSubview(&ornament);
    container.addSubview(&message);
    if let Some(action) = action {
        let action_size = action.frame().size;
        action.setFrame(from_top(
            size,
            (size.width - action_size.width) / 2.0,
            top + FieldOrnament::height() + GAP + message_height + GAP,
            action_size,
        ));
        action.setAutoresizingMask(centered);
        container.addSubview(action);
    }
    container
}
