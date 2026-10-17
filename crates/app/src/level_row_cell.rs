use std::cell::OnceCell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSTableCellView, NSUserInterfaceItemIdentification, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::level_row::LevelRow;
use crate::level_row_view::LevelRowView;
use crate::shell_palette::ShellPalette;

define_class!(
    #[unsafe(super(NSTableCellView, NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = OnceCell<LevelRowView>]
    #[derive(Debug)]
    pub struct LevelRowCell;

    impl LevelRowCell {
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            unsafe {
                let _: () = msg_send![super(self), setFrameSize: size];
            }
            if let Some(parts) = self.ivars().get() {
                parts.layout(size);
            }
        }
    }
);

impl LevelRowCell {
    pub fn new(marker: MainThreadMarker, identifier: &NSString, width: f64, height: f64) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(OnceCell::new());
        let this: Retained<Self> =
            unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, NSSize::new(width, height))] };
        this.setIdentifier(Some(identifier));
        this
    }

    pub fn show(&self, row: &LevelRow, palette: &ShellPalette) {
        if let Some(parts) = self.ivars().get() {
            parts.update(row, palette);
            parts.layout(self.frame().size);
        } else {
            let parts = LevelRowView::build(MainThreadMarker::from(self), self, row, palette);
            let _ = self.ivars().set(parts);
        }
    }
}
