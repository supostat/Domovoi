use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSMenu, NSMenuItem, NSScrollView, NSTableColumn, NSTableView, NSTableViewColumnAutoresizingStyle, NSTableViewStyle,
};
use objc2_foundation::{NSRect, NSString};

use crate::level_list_source::{LevelListModel, LevelListSource, ROW_HEIGHT};
use crate::level_row::LevelRow;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::strings::SHOW_IN_FINDER;

#[derive(Debug)]
pub struct LevelList {
    pub scroll: Retained<NSScrollView>,
    pub table: Retained<NSTableView>,
    pub source: Retained<LevelListSource>,
}

impl LevelList {
    pub fn build(marker: MainThreadMarker, frame: NSRect, palette: ShellPalette, actions: &ShellActions) -> Self {
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(marker), frame);
        scroll.setDrawsBackground(false);
        let table = NSTableView::initWithFrame(NSTableView::alloc(marker), frame);
        table.setStyle(NSTableViewStyle::Plain);
        table.setHeaderView(None);
        table.setBackgroundColor(&palette.tokens.bg.ns_color());
        table.setRowHeight(ROW_HEIGHT);
        table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(marker), &NSString::from_str("level"));
        column.setWidth(frame.size.width);
        table.addTableColumn(&column);
        let menu = NSMenu::new(marker);
        let show = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(marker),
                &NSString::from_str(SHOW_IN_FINDER),
                Some(sel!(showClickedRowInFinder:)),
                &NSString::new(),
            )
        };
        unsafe {
            show.setTarget(Some(actions.target()));
            menu.addItem(&show);
            table.setMenu(Some(&menu));
            table.setTarget(Some(actions.target()));
            table.setDoubleAction(Some(sel!(openClickedRow:)));
        }
        let source = LevelListSource::new(
            marker,
            LevelListModel {
                rows: RefCell::new(Vec::new()),
                palette,
                width: frame.size.width - 4.0,
            },
        );
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            table.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        }
        scroll.setDocumentView(Some(&table));
        table.sizeLastColumnToFit();
        Self { scroll, table, source }
    }

    pub fn show(&self, rows: Vec<LevelRow>) {
        if self.source.show(rows) {
            self.table.reloadData();
        }
    }
}
