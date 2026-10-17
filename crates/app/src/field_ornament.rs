use std::cell::Cell;
use std::ptr::NonNull;
use std::time::Instant;

use block2::RcBlock;
use domovoi_theme::{Motion, Rgba};
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{NSBezierPath, NSImage, NSLineCapStyle, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSTimer};

pub(crate) const CELL: f64 = 5.0;
pub(crate) const MOTIF: [&str; 9] = [
    "....x....",
    "...x.x...",
    "..x.o.x..",
    ".x.o.o.x.",
    "x.o.w.o.x",
    ".x.o.o.x.",
    "..x.o.x..",
    "...x.x...",
    "....x....",
];
const TIE_ROW: usize = 4;
const STROKE_SHARE: f64 = 0.24;
const INSET_SHARE: f64 = 0.2;
const FRAME_INTERVAL: f64 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct OrnamentInk {
    pub outline: Rgba,
    pub fill: Rgba,
    pub center: Rgba,
}

#[derive(Debug)]
pub(crate) struct OrnamentState {
    image: Retained<NSImage>,
    ink: OrnamentInk,
    revealed: Cell<f64>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = OrnamentState]
    #[derive(Debug)]
    pub(crate) struct FieldOrnament;

    impl FieldOrnament {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let state = self.ivars();
            let revealed = state.revealed.get();
            if revealed >= 1.0 {
                state.image.drawInRect(self.bounds());
            } else {
                stitch_field(self.bounds().size, state.ink, revealed);
            }
        }
    }
);

impl FieldOrnament {
    pub(crate) fn height() -> f64 {
        MOTIF.len() as f64 * CELL
    }

    pub(crate) fn new(marker: MainThreadMarker, frame: NSRect, ink: OrnamentInk) -> Retained<Self> {
        let painter = RcBlock::new(move |rect: NSRect| -> Bool {
            stitch_field(rect.size, ink, 1.0);
            Bool::YES
        });
        let image = NSImage::imageWithSize_flipped_drawingHandler(frame.size, true, &painter);
        let this = Self::alloc(marker).set_ivars(OrnamentState {
            image,
            ink,
            revealed: Cell::new(1.0),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub(crate) fn stitch_once(&self, reduce_motion: bool) {
        let duration = Motion::CleanupOrnament.duration(reduce_motion);
        if duration.is_zero() {
            return;
        }
        self.ivars().revealed.set(0.0);
        self.setNeedsDisplay(true);
        let ornament = self.retain();
        let started = Instant::now();
        let tick = RcBlock::new(move |timer: NonNull<NSTimer>| {
            let revealed = (started.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0);
            ornament.ivars().revealed.set(revealed);
            ornament.setNeedsDisplay(true);
            if revealed >= 1.0 {
                unsafe { timer.as_ref() }.invalidate();
            }
        });
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(FRAME_INTERVAL, true, &tick) };
    }
}

fn stitch_field(size: NSSize, ink: OrnamentInk, revealed: f64) {
    let motif_width = MOTIF[0].len();
    let tile_width = motif_width + 1;
    let columns = (size.width / CELL).floor() as usize;
    let tiles = ((columns + 1) / tile_width).max(1);
    let used = tiles * tile_width - 1;
    let offset = ((size.width - used as f64 * CELL) / 2.0).max(0.0);
    let threads = [ink.outline, ink.fill, ink.center].map(|_| NSBezierPath::bezierPath());
    let total = stitch_count(used, tile_width);
    let limit = (total as f64 * revealed).ceil() as usize;
    let mut drawn = 0;
    for column in 0..used {
        let motif_column = column % tile_width;
        for (row, line) in MOTIF.iter().enumerate() {
            let mark = if motif_column < motif_width {
                line.as_bytes()[motif_column]
            } else if row == TIE_ROW {
                b'-'
            } else {
                b'.'
            };
            let thread = match mark {
                b'x' => &threads[0],
                b'o' | b'-' => &threads[1],
                b'w' => &threads[2],
                _ => continue,
            };
            if drawn == limit {
                break;
            }
            drawn += 1;
            cross(thread, offset + column as f64 * CELL, row as f64 * CELL);
        }
    }
    for (thread, color) in threads.iter().zip([ink.outline, ink.fill, ink.center]) {
        color.ns_color().setStroke();
        thread.setLineWidth((CELL * STROKE_SHARE).max(1.0));
        thread.setLineCapStyle(NSLineCapStyle::Round);
        thread.stroke();
    }
}

fn stitch_count(used: usize, tile_width: usize) -> usize {
    let motif_width = MOTIF[0].len();
    (0..used)
        .map(|column| column % tile_width)
        .map(|motif_column| {
            if motif_column < motif_width {
                MOTIF
                    .iter()
                    .filter(|line| line.as_bytes()[motif_column] != b'.')
                    .count()
            } else {
                1
            }
        })
        .sum()
}

fn cross(thread: &NSBezierPath, x: f64, y: f64) {
    let inset = CELL * INSET_SHARE;
    thread.moveToPoint(NSPoint::new(x + inset, y + inset));
    thread.lineToPoint(NSPoint::new(x + CELL - inset, y + CELL - inset));
    thread.moveToPoint(NSPoint::new(x + CELL - inset, y + inset));
    thread.lineToPoint(NSPoint::new(x + inset, y + CELL - inset));
}
