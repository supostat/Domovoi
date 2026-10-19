use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSLayoutPriorityDefaultHigh, NSSplitViewController, NSSplitViewItem, NSView,
    NSViewController,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::fill_view::{FillStyle, FillView};
use crate::navigation::Navigation;
use crate::section::Section;
use crate::shell_actions::ShellActions;
use crate::shell_data::ShellData;
use crate::shell_layout::{SIDEBAR_WIDTH, WINDOW};
use crate::shell_palette::ShellPalette;
use crate::shell_split_view::ShellSplitView;
use crate::sidebar_panel::SidebarPanel;

const DIVIDER: f64 = 1.0;

#[derive(Debug)]
pub struct WindowFrame {
    pub controller: Retained<NSSplitViewController>,
    pub sidebar: SidebarPanel,
    pub pane: Retained<FillView>,
}

impl WindowFrame {
    pub fn build(
        marker: MainThreadMarker,
        data: &ShellData,
        current: Section,
        palette: &ShellPalette,
        actions: &ShellActions,
    ) -> Self {
        let sidebar = SidebarPanel::build(
            marker,
            NSSize::new(SIDEBAR_WIDTH, WINDOW.height),
            data,
            Navigation {
                current,
                care_ready: data.care_ready,
            },
            *palette,
            actions,
        );
        let pane = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, Self::pane_size()),
            FillStyle {
                fill: Some(palette.tokens.bg.ns_color()),
                border: None,
                radius: 0.0,
                dashed: false,
            },
        );
        let split = ShellSplitView::new(
            marker,
            NSRect::new(NSPoint::ZERO, WINDOW),
            palette.line(),
            palette.tokens.bg.ns_color(),
        );
        let controller = NSSplitViewController::new(marker);
        controller.setSplitView(&split);
        let sidebar_item = NSSplitViewItem::splitViewItemWithViewController(&hosting(marker, &sidebar.view));
        sidebar_item.setMinimumThickness(SIDEBAR_WIDTH);
        sidebar_item.setMaximumThickness(SIDEBAR_WIDTH);
        sidebar_item.setCanCollapse(false);
        sidebar_item.setHoldingPriority(NSLayoutPriorityDefaultHigh);
        controller.addSplitViewItem(&sidebar_item);
        controller.addSplitViewItem(&NSSplitViewItem::splitViewItemWithViewController(&hosting(
            marker, &pane,
        )));
        let root = controller.view();
        root.setFrame(NSRect::new(NSPoint::ZERO, WINDOW));
        root.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        split.setPosition_ofDividerAtIndex(SIDEBAR_WIDTH, 0);
        root.layoutSubtreeIfNeeded();
        Self {
            controller,
            sidebar,
            pane,
        }
    }

    pub fn root(&self) -> Retained<NSView> {
        self.controller.view()
    }

    pub fn pane_size() -> NSSize {
        NSSize::new(WINDOW.width - SIDEBAR_WIDTH - DIVIDER, WINDOW.height)
    }

    pub fn pane_origin_x() -> f64 {
        SIDEBAR_WIDTH + DIVIDER
    }
}

fn hosting(marker: MainThreadMarker, view: &NSView) -> Retained<NSViewController> {
    let controller = NSViewController::new(marker);
    controller.setView(view);
    controller
}
