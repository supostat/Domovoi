use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};

use objc2_app_kit::{
    NSControlTextEditingDelegate, NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSView,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{NSInteger, NSObject, NSString};

use crate::level_row::LevelRow;
use crate::level_row_cell::LevelRowCell;
use crate::shell_palette::ShellPalette;

pub(crate) const ROW_HEIGHT: CGFloat = 44.0;
const REUSE_IDENTIFIER: &str = "level-row";

#[derive(Debug)]
pub struct LevelListModel {
    pub rows: RefCell<Vec<LevelRow>>,
    pub palette: ShellPalette,
    pub width: f64,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = LevelListModel]
    #[derive(Debug)]
    pub struct LevelListSource;

    unsafe impl NSObjectProtocol for LevelListSource {}

    unsafe impl NSTableViewDataSource for LevelListSource {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, _table: &NSTableView) -> NSInteger {
            NSInteger::try_from(self.ivars().rows.borrow().len()).expect("a level has few rows")
        }
    }

    unsafe impl NSControlTextEditingDelegate for LevelListSource {}

    unsafe impl NSTableViewDelegate for LevelListSource {
        #[unsafe(method(tableView:heightOfRow:))]
        fn row_height(&self, _table: &NSTableView, _row: NSInteger) -> CGFloat {
            ROW_HEIGHT
        }

        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(
            &self,
            table: &NSTableView,
            _column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<NSView>> {
            let model = self.ivars();
            let rows = model.rows.borrow();
            usize::try_from(row).ok().and_then(|row| rows.get(row)).map(|row| {
                let identifier = NSString::from_str(REUSE_IDENTIFIER);
                let cell = unsafe { table.makeViewWithIdentifier_owner(&identifier, None) }
                    .and_then(|view| view.downcast::<LevelRowCell>().ok())
                    .unwrap_or_else(|| {
                        LevelRowCell::new(MainThreadMarker::from(self), &identifier, model.width, ROW_HEIGHT)
                    });
                cell.show(row, &model.palette);
                cell.into_super().into_super()
            })
        }
    }
);

impl LevelListSource {
    pub fn new(marker: MainThreadMarker, model: LevelListModel) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(model);
        unsafe { msg_send![super(this), init] }
    }

    pub fn show(&self, rows: Vec<LevelRow>) -> bool {
        let model = self.ivars();
        if *model.rows.borrow() == rows {
            return false;
        }
        model.rows.replace(rows);
        true
    }
}
