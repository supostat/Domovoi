use objc2::rc::Retained;
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
use crate::strings::SHOW_IN_FINDER;
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
        let below = if has_path { BUTTON_HEIGHT + BUTTON_GAP } else { 0.0 };
        let height = labels.lay_out(facts, tokens, WIDTH, PADDING, LINE_GAP, below);
        if has_path {
            card.addSubview(&finder_button(marker, facts.node, palette, actions));
        }
        card.setFrameSize(NSSize::new(WIDTH, height));
        self.controller.setView(&card);
        self.popover.setContentSize(NSSize::new(WIDTH, height));
        self.popover
            .showRelativeToRect_ofView_preferredEdge(rect, anchor, NSRectEdge::MinY);
    }
}

fn finder_button(
    marker: MainThreadMarker,
    node: usize,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Retained<ShellButton> {
    let finder = ShellButton::new(
        marker,
        SHOW_IN_FINDER,
        NSPoint::new(PADDING, PADDING),
        BUTTON_HEIGHT,
        ButtonStyle {
            fill: None,
            border: Some(palette.line()),
            text: palette.tokens.text.ns_color(),
            font: interface_font(12.5, 500.0),
        },
    );
    finder.on_click(actions, sel!(showNodeInFinder:));
    finder.setTag(NSInteger::try_from(node).expect("a node position fits a tag"));
    finder
}
