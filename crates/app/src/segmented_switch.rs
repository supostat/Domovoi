use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2_app_kit::NSAccessibility;
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize};

use crate::button_style::ButtonStyle;
use crate::fill_view::{FillStyle, FillView};
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_palette::ShellPalette;
use crate::ui_font::interface_font;

const HEIGHT: f64 = 30.0;
const INSET: f64 = 3.0;
const RADIUS: f64 = 9.0;

#[derive(Debug)]
pub struct SegmentedSwitch {
    pub view: Retained<FillView>,
    pub segments: Vec<Retained<ShellButton>>,
}

impl SegmentedSwitch {
    pub(crate) fn build(
        marker: MainThreadMarker,
        options: &[(&str, bool)],
        selected: usize,
        palette: &ShellPalette,
        action: (&ShellActions, Sel),
    ) -> Self {
        let tokens = palette.tokens;
        let (actions, selector) = action;
        let segments: Vec<Retained<ShellButton>> = options
            .iter()
            .enumerate()
            .map(|(position, (title, enabled))| {
                let chosen = position == selected;
                let button = ShellButton::new(
                    marker,
                    title,
                    NSPoint::ZERO,
                    HEIGHT - 2.0 * INSET,
                    ButtonStyle {
                        fill: chosen.then(|| tokens.surface_2.ns_color()),
                        border: None,
                        text: if chosen { tokens.text } else { tokens.mute }.ns_color(),
                        font: interface_font(13.0, 500.0),
                    },
                );
                button.setTag(NSInteger::try_from(position).expect("a switch has few segments"));
                button.on_click(actions, selector);
                button.setEnabled(*enabled);
                button.setAccessibilitySelected(chosen);
                button
            })
            .collect();
        let width = segments.iter().map(|segment| segment.frame().size.width).sum::<f64>() + 2.0 * INSET;
        let view = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(width, HEIGHT)),
            FillStyle {
                fill: Some(tokens.surface.ns_color()),
                border: Some(palette.line_soft()),
                radius: RADIUS,
                dashed: false,
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
