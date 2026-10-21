use domovoi_render::DiagramMode;
use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSBackingStoreType, NSScrollView, NSTableView, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect};

use crate::main_screen::MainScreen;
use crate::shell_actions::ShellActions;
use crate::shell_data::ShellData;
use crate::shell_layout::WINDOW;
use crate::shell_palette::ShellPalette;

#[derive(Debug)]
pub(crate) struct DiskProbe {
    window: Retained<NSWindow>,
    screen: MainScreen,
    scroll: Retained<NSScrollView>,
    table: Retained<NSTableView>,
    _actions: Retained<ShellActions>,
}

impl DiskProbe {
    pub(crate) fn scrolled_to_the_end(marker: MainThreadMarker, data: &ShellData, focus: usize) -> Self {
        let _application = NSApplication::sharedApplication(marker);
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(marker),
                NSRect::new(NSPoint::ZERO, WINDOW),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        let actions = ShellActions::new(marker);
        let palette = ShellPalette {
            tokens: &Palette::DARK,
            high_contrast: false,
        };
        let screen = MainScreen::build(marker, data, focus, DiagramMode::Sunburst, &palette, &actions);
        window.setContentView(Some(&screen.frame.root()));
        let probe = Self {
            scroll: screen.list.scroll.clone(),
            table: screen.list.table.clone(),
            window,
            screen,
            _actions: actions,
        };
        probe.lay_out();
        let clip = probe.scroll.contentView();
        let end = (probe.table.frame().size.height - clip.bounds().size.height).max(0.0);
        clip.scrollToPoint(NSPoint::new(0.0, end));
        probe.scroll.reflectScrolledClipView(&clip);
        probe
    }

    pub(crate) fn update(&mut self, data: &ShellData, focus: usize) {
        self.screen.update(data, focus);
        self.lay_out();
    }

    pub(crate) fn rows(&self) -> isize {
        self.screen.list.table.numberOfRows()
    }

    pub(crate) fn offset(&self) -> f64 {
        self.scroll.contentView().bounds().origin.y
    }

    pub(crate) fn keeps_its_scroll(&self) -> bool {
        let shown_in_window = self
            .scroll
            .window()
            .is_some_and(|window| std::ptr::eq(Retained::as_ptr(&window), Retained::as_ptr(&self.window)));
        let encloses_the_list = self
            .screen
            .list
            .table
            .enclosingScrollView()
            .is_some_and(|scroll| std::ptr::eq(Retained::as_ptr(&scroll), Retained::as_ptr(&self.scroll)));
        shown_in_window && encloses_the_list
    }

    pub(crate) fn keeps_its_table(&self) -> bool {
        std::ptr::eq(Retained::as_ptr(&self.screen.list.table), Retained::as_ptr(&self.table))
    }

    fn lay_out(&self) {
        self.scroll.tile();
        self.window
            .contentView()
            .expect("the window shows the disk screen")
            .layoutSubtreeIfNeeded();
        self.table.layoutSubtreeIfNeeded();
    }
}
