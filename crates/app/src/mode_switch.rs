use domovoi_render::DiagramMode;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::NSAccessibility;
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize};

use crate::button_style::ButtonStyle;
use crate::fill_view::{FillStyle, FillView};
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_palette::ShellPalette;
use crate::strings::{SUNBURST, TREEMAP};
use crate::ui_font::interface_font;

const HEIGHT: f64 = 30.0;
const INSET: f64 = 3.0;
const RADIUS: f64 = 9.0;
pub(crate) const SUNBURST_TAG: NSInteger = 0;
pub(crate) const TREEMAP_TAG: NSInteger = 1;

#[derive(Debug)]
pub struct ModeSwitch {
    pub view: Retained<FillView>,
    pub segments: [Retained<ShellButton>; 2],
}

impl ModeSwitch {
    pub fn build(marker: MainThreadMarker, mode: DiagramMode, palette: &ShellPalette, actions: &ShellActions) -> Self {
        let tokens = palette.tokens;
        let segment = |title: &str, tag: NSInteger, selected: bool| {
            let button = ShellButton::new(
                marker,
                title,
                NSPoint::ZERO,
                HEIGHT - 2.0 * INSET,
                ButtonStyle {
                    fill: selected.then(|| tokens.surface_2.ns_color()),
                    border: None,
                    text: if selected { tokens.text } else { tokens.mute }.ns_color(),
                    font: interface_font(13.0, 500.0),
                },
            );
            button.setTag(tag);
            button.on_click(actions, sel!(selectMode:));
            button.setAccessibilitySelected(selected);
            button
        };
        let segments = [
            segment(SUNBURST, SUNBURST_TAG, mode == DiagramMode::Sunburst),
            segment(TREEMAP, TREEMAP_TAG, mode == DiagramMode::Treemap),
        ];
        let width = segments.iter().map(|segment| segment.frame().size.width).sum::<f64>() + 2.0 * INSET;
        let view = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(width, HEIGHT)),
            FillStyle {
                fill: Some(tokens.surface.ns_color()),
                border: Some(palette.line_soft()),
                radius: RADIUS,
            },
        );
        let mut left = INSET;
        for segment in &segments {
            segment.setFrameOrigin(NSPoint::new(left, INSET));
            left += segment.frame().size.width;
            view.addSubview(segment);
        }
        Self { view, segments }
    }
}
