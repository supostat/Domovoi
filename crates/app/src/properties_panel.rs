use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSPopover, NSPopoverBehavior, NSView, NSViewController};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSRectEdge, NSSize};

use crate::button_style::ButtonStyle;
use crate::fill_view::{FillStyle, FillView};
use crate::node_facts::NodeFacts;
use crate::property_labels::PropertyLabels;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_palette::ShellPalette;
use crate::strings::{REMOVE_WITH_FILES, SHOW_IN_FINDER};
use crate::ui_font::interface_font;

const WIDTH: f64 = 340.0;
const PADDING: f64 = 14.0;
const LINE_GAP: f64 = 4.0;
const BUTTON_HEIGHT: f64 = 28.0;
const BUTTON_GAP: f64 = 12.0;

#[derive(Debug)]
pub(crate) struct PropertiesPanel {
    popover: Retained<NSPopover>,
    controller: Retained<NSViewController>,
}

impl PropertiesPanel {
    pub(crate) fn new(marker: MainThreadMarker) -> Self {
        let popover = NSPopover::new(marker);
        popover.setBehavior(NSPopoverBehavior::Transient);
        let controller = NSViewController::new(marker);
        popover.setContentViewController(Some(&controller));
        Self { popover, controller }
    }

    pub(crate) fn show(
        &self,
        marker: MainThreadMarker,
        facts: &NodeFacts,
        palette: &ShellPalette,
        actions: &ShellActions,
        anchor: &NSView,
        rect: NSRect,
    ) {
        let tokens = palette.tokens;
        let card = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, 1.0)),
            FillStyle {
                fill: Some(tokens.surface.ns_color()),
                border: None,
                radius: 0.0,
                dashed: false,
            },
        );
        let labels = PropertyLabels::new(marker, WIDTH - 2.0 * PADDING);
        labels.attach(&card);
        let has_path = facts.tip.path.is_some();
        let removable = facts
            .indexed
            .as_ref()
            .is_some_and(|indexed| indexed.removable_application);
        let buttons: Vec<(&str, Sel)> = [
            (has_path, SHOW_IN_FINDER, sel!(showNodeInFinder:)),
            (removable, REMOVE_WITH_FILES, sel!(removeApplication:)),
        ]
        .into_iter()
        .filter_map(|(shown, title, selector)| shown.then_some((title, selector)))
        .collect();
        let below = buttons.len() as f64 * (BUTTON_HEIGHT + BUTTON_GAP);
        let height = labels.lay_out(facts, tokens, WIDTH, PADDING, LINE_GAP, below);
        let mut bottom = PADDING;
        for (title, selector) in buttons.into_iter().rev() {
            card.addSubview(&panel_button(
                marker,
                (title, selector),
                facts.node,
                bottom,
                palette,
                actions,
            ));
            bottom += BUTTON_HEIGHT + BUTTON_GAP;
        }
        card.setFrameSize(NSSize::new(WIDTH, height));
        self.controller.setView(&card);
        self.popover.setContentSize(NSSize::new(WIDTH, height));
        self.popover
            .showRelativeToRect_ofView_preferredEdge(rect, anchor, NSRectEdge::MinY);
    }

    pub(crate) fn close(&self) {
        self.popover.close();
    }
}

fn panel_button(
    marker: MainThreadMarker,
    (title, selector): (&str, Sel),
    node: usize,
    bottom: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Retained<ShellButton> {
    let button = ShellButton::new(
        marker,
        title,
        NSPoint::new(PADDING, bottom),
        BUTTON_HEIGHT,
        ButtonStyle {
            fill: None,
            border: Some(palette.line()),
            text: palette.tokens.text.ns_color(),
            font: interface_font(12.5, 500.0),
        },
    );
    button.on_click(actions, selector);
    button.setTag(NSInteger::try_from(node).expect("a node position fits a tag"));
    button
}
