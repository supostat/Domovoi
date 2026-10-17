use block2::RcBlock;
use domovoi_theme::Rgba;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBezierPath, NSImage, NSLineCapStyle, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

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

#[derive(Clone, Copy, Debug)]
pub(crate) struct OrnamentInk {
    pub outline: Rgba,
    pub fill: Rgba,
    pub center: Rgba,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Retained<NSImage>]
    #[derive(Debug)]
    pub struct FieldOrnament;

    impl FieldOrnament {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.ivars().drawInRect(self.bounds());
        }
    }
);

impl FieldOrnament {
    pub(crate) fn height() -> f64 {
        MOTIF.len() as f64 * CELL
    }

    pub(crate) fn new(marker: MainThreadMarker, frame: NSRect, ink: OrnamentInk) -> Retained<Self> {
        let painter = RcBlock::new(move |rect: NSRect| -> Bool {
            stitch_field(rect.size, ink);
            Bool::YES
        });
        let image = NSImage::imageWithSize_flipped_drawingHandler(frame.size, true, &painter);
        let this = Self::alloc(marker).set_ivars(image);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

fn stitch_field(size: NSSize, ink: OrnamentInk) {
    let motif_width = MOTIF[0].len();
    let tile_width = motif_width + 1;
    let columns = (size.width / CELL).floor() as usize;
    let tiles = ((columns + 1) / tile_width).max(1);
    let used = tiles * tile_width - 1;
    let offset = ((size.width - used as f64 * CELL) / 2.0).max(0.0);
    let threads = [ink.outline, ink.fill, ink.center].map(|_| NSBezierPath::bezierPath());
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

fn cross(thread: &NSBezierPath, x: f64, y: f64) {
    let inset = CELL * INSET_SHARE;
    thread.moveToPoint(NSPoint::new(x + inset, y + inset));
    thread.lineToPoint(NSPoint::new(x + CELL - inset, y + CELL - inset));
    thread.moveToPoint(NSPoint::new(x + CELL - inset, y + inset));
    thread.lineToPoint(NSPoint::new(x + inset, y + CELL - inset));
}
