mod support;

use std::process::ExitCode;

use domovoi_render::{BgraImage, DiagramRenderer, SHAPE_RECT, Scene, ShapeInstance};
use domovoi_theme::{Palette, Rgba};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBezierPath, NSColor, NSGraphicsContext, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use support::renderer;

const SIDE: f32 = 16.0;
const PROBE: usize = 8;
const CHANNEL_TOLERANCE: u8 = 1;

struct SwatchColors {
    background: Retained<NSColor>,
    fill: Retained<NSColor>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SwatchColors]
    struct SwatchView;

    impl SwatchView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            self.ivars().background.setFill();
            NSBezierPath::fillRect(bounds);
            self.ivars().fill.setFill();
            NSBezierPath::fillRect(bounds);
        }
    }
);

fn view_pixel(marker: MainThreadMarker, background: Rgba, fill: Rgba) -> [u8; 3] {
    let view = SwatchView::alloc(marker).set_ivars(SwatchColors {
        background: background.ns_color(),
        fill: fill.ns_color(),
    });
    let frame = NSRect::new(NSPoint::ZERO, NSSize::new(f64::from(SIDE), f64::from(SIDE)));
    let view: Retained<SwatchView> = unsafe { msg_send![super(view), initWithFrame: frame] };
    let bounds = view.bounds();
    let side = SIDE as usize;
    let image = BgraImage::draw(side, side, |context| {
        let graphics = NSGraphicsContext::graphicsContextWithCGContext_flipped(context, false);
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&graphics));
        view.displayRectIgnoringOpacity_inContext(bounds, &graphics);
        NSGraphicsContext::restoreGraphicsState_class();
    });
    let [blue, green, red, _] = image.pixel(PROBE, PROBE);
    [red, green, blue]
}

fn metal_pixel(renderer: &mut DiagramRenderer, background: Rgba, fill: Rgba) -> [u8; 3] {
    let geometry = [0.0, 0.0, SIDE, SIDE];
    let scene = Scene {
        width: SIDE,
        height: SIDE,
        scale: 1.0,
        progress: 1.0,
        clear: background.premultiplied(),
        shapes: vec![ShapeInstance {
            geometry,
            previous: geometry,
            fill: fill.premultiplied(),
            kind: SHAPE_RECT,
            ..ShapeInstance::default()
        }],
        glyphs: Vec::new(),
    };
    let atlas = domovoi_render::GlyphAtlas::default();
    let [blue, green, red, _] = renderer.render_offscreen(&scene, &atlas).image.pixel(PROBE, PROBE);
    [red, green, blue]
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the parity check runs on the main thread");
    let mut renderer = renderer();
    let mut failures = Vec::new();
    let mut checks = 0;
    for (theme, palette) in [("dark", &Palette::DARK), ("light", &Palette::LIGHT)] {
        let tokens = [
            ("wax", palette.wax),
            ("c_apps", palette.c_apps),
            ("wax_soft", palette.wax_soft),
            ("ok_soft", palette.ok_soft),
            ("warn_soft", palette.warn_soft),
            ("risk_soft", palette.risk_soft),
        ];
        for (token, color) in tokens {
            let metal = metal_pixel(&mut renderer, palette.bg, color);
            let view = view_pixel(marker, palette.bg, color);
            checks += 1;
            if metal
                .iter()
                .zip(view)
                .any(|(left, right)| left.abs_diff(right) > CHANNEL_TOLERANCE)
            {
                failures.push(format!("{theme} {token}: Metal {metal:?}, NSView {view:?}"));
            }
        }
    }
    if failures.is_empty() {
        println!("color parity: {checks} token checks passed");
        ExitCode::SUCCESS
    } else {
        eprintln!("color parity failed:\n{}", failures.join("\n"));
        ExitCode::FAILURE
    }
}
