use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSOutlineView, NSScrollView, NSTableColumn, NSTableViewColumnAutoresizingStyle, NSTableViewSelectionHighlightStyle,
    NSTableViewStyle, NSTextField,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::fill_view::{FillStyle, FillView};
use crate::label::measure_label;
use crate::navigation::Navigation;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_palette::ShellPalette;
use crate::sidebar_entry::SidebarEntry;
use crate::sidebar_source::{SidebarModel, SidebarSource};
use crate::strings::RESCAN;
use crate::ui_font::{interface_font, mono_font};

const DISK_BLOCK: f64 = 96.0;
const INSET: f64 = 14.0;
const FREE_BOTTOM: f64 = 10.0;
const BAR_BOTTOM: f64 = 36.0;
const BAR_HEIGHT: f64 = 6.0;
const RESCAN_BOTTOM: f64 = 54.0;
const RESCAN_HEIGHT: f64 = 28.0;

#[derive(Debug)]
pub struct SidebarPanel {
    pub view: Retained<FillView>,
    pub outline: Retained<NSOutlineView>,
    pub source: Retained<SidebarSource>,
    pub free: Retained<NSTextField>,
    pub rescan: Retained<ShellButton>,
}

impl SidebarPanel {
    pub fn build(
        marker: MainThreadMarker,
        size: NSSize,
        data: &ShellData,
        navigation: Navigation,
        palette: ShellPalette,
        actions: &ShellActions,
    ) -> Self {
        let groups = data.sidebar(navigation);
        let tokens = palette.tokens;
        let view = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, size),
            FillStyle {
                fill: Some(tokens.bg_2.ns_color()),
                border: None,
                radius: 0.0,
                dashed: false,
            },
        );
        let bar_track = FillView::new(
            marker,
            NSRect::new(
                NSPoint::new(INSET, BAR_BOTTOM),
                NSSize::new(size.width - 2.0 * INSET, BAR_HEIGHT),
            ),
            FillStyle {
                fill: Some(palette.line_soft()),
                border: None,
                radius: BAR_HEIGHT / 2.0,
                dashed: false,
            },
        );
        let bar_fill = FillView::new(
            marker,
            NSRect::new(
                NSPoint::new(INSET, BAR_BOTTOM),
                NSSize::new(
                    (size.width - 2.0 * INSET) * data.used_share().clamp(0.0, 1.0),
                    BAR_HEIGHT,
                ),
            ),
            FillStyle {
                fill: Some(tokens.wax.ns_color()),
                border: None,
                radius: BAR_HEIGHT / 2.0,
                dashed: false,
            },
        );
        let free = measure_label(
            marker,
            &data.free_line(),
            &mono_font(11.5, 400.0),
            &tokens.dim.ns_color(),
        );
        free.setFrameOrigin(NSPoint::new(INSET, FREE_BOTTOM));
        let rescan = rescan_button(marker, palette, actions, data.rescan_allowed);
        view.addSubview(&bar_track);
        view.addSubview(&bar_fill);
        view.addSubview(&free);
        view.addSubview(&rescan);

        let outline_frame = NSRect::new(
            NSPoint::new(0.0, DISK_BLOCK),
            NSSize::new(size.width, size.height - DISK_BLOCK),
        );
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(marker), outline_frame);
        scroll.setDrawsBackground(false);
        let outline = NSOutlineView::initWithFrame(NSOutlineView::alloc(marker), outline_frame);
        outline.setStyle(NSTableViewStyle::SourceList);
        outline.setHeaderView(None);
        outline.setBackgroundColor(&tokens.bg_2.ns_color());
        outline.setFloatsGroupRows(false);
        outline.setSelectionHighlightStyle(NSTableViewSelectionHighlightStyle::None);
        outline.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(marker), &NSString::from_str("sidebar"));
        column.setWidth(size.width);
        outline.addTableColumn(&column);
        unsafe { outline.setOutlineTableColumn(Some(&column)) };
        let selected = selected_row(&groups);
        let source = SidebarSource::new(marker, SidebarModel::new(groups, palette, size.width, actions.retain()));
        unsafe {
            outline.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            outline.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        }
        outline.reloadData();
        unsafe { outline.expandItem_expandChildren(None, true) };
        if let Some(row) = selected {
            outline.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
        scroll.setDocumentView(Some(&outline));
        outline.sizeLastColumnToFit();
        view.addSubview(&scroll);
        Self {
            view,
            outline,
            source,
            free,
            rescan,
        }
    }
}

fn rescan_button(
    marker: MainThreadMarker,
    palette: ShellPalette,
    actions: &ShellActions,
    enabled: bool,
) -> Retained<ShellButton> {
    let rescan = ShellButton::new(
        marker,
        RESCAN,
        NSPoint::new(INSET, RESCAN_BOTTOM),
        RESCAN_HEIGHT,
        ButtonStyle {
            fill: None,
            border: Some(palette.line()),
            text: palette.tokens.text.ns_color(),
            font: interface_font(13.0, 500.0),
        },
    );
    rescan.on_click(actions, sel!(rescanDisk:));
    rescan.setEnabled(enabled);
    rescan
}

fn selected_row(groups: &[SidebarEntry]) -> Option<usize> {
    let mut row = 0;
    for group in groups {
        row += 1;
        for child in &group.children {
            if child.selected {
                return Some(row);
            }
            row += 1;
        }
    }
    None
}
