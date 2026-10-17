use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSOutlineView, NSScrollView, NSTableColumn, NSTableViewColumnAutoresizingStyle, NSTableViewSelectionHighlightStyle,
    NSTableViewStyle, NSTextField,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::fill_view::{FillStyle, FillView};
use crate::label::measure_label;
use crate::shell_palette::ShellPalette;
use crate::sidebar_entry::SidebarEntry;
use crate::sidebar_source::{SidebarModel, SidebarSource};
use crate::ui_font::mono_font;

const DISK_BLOCK: f64 = 56.0;
const INSET: f64 = 14.0;
const BAR_HEIGHT: f64 = 6.0;

#[derive(Debug)]
pub struct SidebarPanel {
    pub view: Retained<FillView>,
    pub outline: Retained<NSOutlineView>,
    pub source: Retained<SidebarSource>,
    pub free: Retained<NSTextField>,
}

impl SidebarPanel {
    pub fn build(
        marker: MainThreadMarker,
        size: NSSize,
        groups: Vec<SidebarEntry>,
        used_share: f64,
        free_text: &str,
        palette: ShellPalette,
    ) -> Self {
        let tokens = palette.tokens;
        let view = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, size),
            FillStyle {
                fill: Some(tokens.bg_2.ns_color()),
                border: None,
                radius: 0.0,
            },
        );
        let bar_track = FillView::new(
            marker,
            NSRect::new(
                NSPoint::new(INSET, DISK_BLOCK - 20.0),
                NSSize::new(size.width - 2.0 * INSET, BAR_HEIGHT),
            ),
            FillStyle {
                fill: Some(palette.line_soft()),
                border: None,
                radius: BAR_HEIGHT / 2.0,
            },
        );
        let bar_fill = FillView::new(
            marker,
            NSRect::new(
                NSPoint::new(INSET, DISK_BLOCK - 20.0),
                NSSize::new((size.width - 2.0 * INSET) * used_share.clamp(0.0, 1.0), BAR_HEIGHT),
            ),
            FillStyle {
                fill: Some(tokens.wax.ns_color()),
                border: None,
                radius: BAR_HEIGHT / 2.0,
            },
        );
        let free = measure_label(marker, free_text, &mono_font(11.5, 400.0), &tokens.dim.ns_color());
        free.setFrameOrigin(NSPoint::new(INSET, 10.0));
        view.addSubview(&bar_track);
        view.addSubview(&bar_fill);
        view.addSubview(&free);

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
        let source = SidebarSource::new(marker, SidebarModel::new(groups, palette, size.width));
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
        }
    }
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
