use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};

#[derive(Debug)]
pub(crate) struct StackedColumn {
    pub(crate) x: f64,
    pub(crate) width: f64,
    pub(crate) top: f64,
    placed: Vec<(Retained<NSView>, f64, f64, NSSize)>,
}

impl StackedColumn {
    pub(crate) fn new(x: f64, width: f64, top: f64) -> Self {
        Self {
            x,
            width,
            top,
            placed: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, view: Retained<NSView>, height: f64) {
        self.placed
            .push((view, self.x, self.top, NSSize::new(self.width, height)));
        self.top += height;
    }

    pub(crate) fn push_beside(&mut self, view: Retained<NSView>, x: f64, size: NSSize) {
        self.placed.push((view, x, self.top, size));
    }

    pub(crate) fn skip(&mut self, gap: f64) {
        self.top += gap;
    }

    pub(crate) fn place_into(self, container: &NSView, height: f64) {
        for (view, x, top, size) in self.placed {
            view.setFrame(NSRect::new(NSPoint::new(x, height - top - size.height), size));
            container.addSubview(&view);
        }
    }
}
