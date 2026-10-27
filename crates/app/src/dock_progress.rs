use std::cell::Cell;

use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSBezierPath, NSView};
use objc2_foundation::{NSBundle, NSPoint, NSRect, NSSize};

pub const BAR_INSET: f64 = 14.0;
pub const BAR_HEIGHT: f64 = 10.0;
const BAR_BOTTOM: f64 = 16.0;
const TRACK_OPACITY: f64 = 0.9;
const TILE_PALETTE: &Palette = &Palette::DARK;

pub fn bar_track(tile: NSSize) -> NSRect {
    NSRect::new(
        NSPoint::new(BAR_INSET, BAR_BOTTOM),
        NSSize::new(tile.width - 2.0 * BAR_INSET, BAR_HEIGHT),
    )
}

pub fn bar_frame(sweep: f32, tile: NSSize) -> NSRect {
    let track = bar_track(tile);
    NSRect::new(
        track.origin,
        NSSize::new(track.size.width * f64::from(sweep.clamp(0.0, 1.0)), BAR_HEIGHT),
    )
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Cell<f32>]
    #[derive(Debug)]
    pub(crate) struct DockProgressBar;

    impl DockProgressBar {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            if let Some(icon) = NSApplication::sharedApplication(MainThreadMarker::from(self)).applicationIconImage() {
                icon.drawInRect(bounds);
            }
            let radius = BAR_HEIGHT / 2.0;
            let track = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bar_track(bounds.size), radius, radius);
            TILE_PALETTE
                .surface_2
                .ns_color()
                .colorWithAlphaComponent(TRACK_OPACITY)
                .setFill();
            track.fill();
            let bar = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                bar_frame(self.ivars().get(), bounds.size),
                radius,
                radius,
            );
            TILE_PALETTE.wax.ns_color().setFill();
            bar.fill();
        }
    }
);

impl DockProgressBar {
    fn new(marker: MainThreadMarker, size: NSSize) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(Cell::new(0.0));
        unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, size)] }
    }
}

#[derive(Debug)]
pub(crate) struct DockProgress {
    marker: MainThreadMarker,
    bar: Option<Retained<DockProgressBar>>,
}

impl DockProgress {
    pub(crate) fn new(marker: MainThreadMarker) -> Self {
        Self { marker, bar: None }
    }

    pub(crate) fn show(&mut self, sweep: f32) {
        if NSBundle::mainBundle().bundleIdentifier().is_none() {
            return;
        }
        let tile = NSApplication::sharedApplication(self.marker).dockTile();
        let bar = self
            .bar
            .get_or_insert_with(|| DockProgressBar::new(self.marker, tile.size()));
        if tile.contentView(self.marker).is_none() {
            tile.setContentView(Some(bar));
        }
        bar.ivars().set(sweep);
        tile.display();
    }

    pub(crate) fn clear(&mut self) {
        if self.bar.take().is_none() {
            return;
        }
        let tile = NSApplication::sharedApplication(self.marker).dockTile();
        tile.setContentView(None);
        tile.display();
    }
}
