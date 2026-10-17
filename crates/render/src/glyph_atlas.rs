use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicU64, Ordering};

use objc2_core_foundation::{CGPoint, CGRect};
use objc2_core_graphics::{CGBitmapContextCreate, CGColorSpace, CGContext, CGImageAlphaInfo, CGImageByteOrderInfo};
use objc2_core_text::{CTFont, CTFontOrientation};

use crate::atlas_entry::AtlasEntry;
use crate::positioned_glyph::PositionedGlyph;

pub const ATLAS_SIDE: u32 = 2048;
const PADDING: i32 = 1;
const SUBPIXEL_VARIANTS: f32 = 4.0;
const BYTES_PER_PIXEL: usize = 4;
const GREEN_CHANNEL: usize = 1;
static NEXT_ATLAS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: String,
    size_bits: u32,
    glyph: u16,
    variant: u8,
}

#[derive(Clone, Copy, Debug)]
struct Shelf {
    y: u32,
    height: u32,
    cursor: u32,
}

#[derive(Debug)]
pub struct GlyphAtlas {
    identity: u64,
    pixels: Vec<u8>,
    shelves: Vec<Shelf>,
    entries: HashMap<GlyphKey, Option<AtlasEntry>>,
    revision: u64,
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self {
            identity: NEXT_ATLAS.fetch_add(1, Ordering::Relaxed),
            pixels: vec![0; (ATLAS_SIDE * ATLAS_SIDE) as usize],
            shelves: Vec::new(),
            entries: HashMap::new(),
            revision: 1,
        }
    }
}

impl GlyphAtlas {
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn version(&self) -> (u64, u64) {
        (self.identity, self.revision)
    }

    pub fn place(&mut self, glyph: &PositionedGlyph, pen_x: f32, scale: f32) -> (i32, Option<AtlasEntry>) {
        let mut whole = pen_x.floor();
        let mut variant = ((pen_x - whole) * SUBPIXEL_VARIANTS).round() as u8;
        if f32::from(variant) >= SUBPIXEL_VARIANTS {
            whole += 1.0;
            variant = 0;
        }
        let device_size = unsafe { glyph.font.size() } as f32 * scale;
        let key = GlyphKey {
            font: unsafe { glyph.font.post_script_name() }.to_string(),
            size_bits: device_size.to_bits(),
            glyph: glyph.glyph,
            variant,
        };
        if let Some(entry) = self.entries.get(&key) {
            return (whole as i32, *entry);
        }
        let entry = self.rasterize(
            &glyph.font,
            glyph.glyph,
            device_size,
            f32::from(variant) / SUBPIXEL_VARIANTS,
        );
        self.entries.insert(key, entry);
        (whole as i32, entry)
    }

    fn rasterize(&mut self, font: &CTFont, glyph: u16, device_size: f32, offset: f32) -> Option<AtlasEntry> {
        let device_font = unsafe { font.copy_with_attributes(f64::from(device_size), ptr::null(), None) };
        let mut glyph_id = glyph;
        let mut bounds = CGRect::default();
        unsafe {
            device_font.bounding_rects_for_glyphs(
                CTFontOrientation::Default,
                NonNull::from(&mut glyph_id),
                &raw mut bounds,
                1,
            );
        }
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return None;
        }
        let left = bounds.origin.x.floor() as i32;
        let right = (bounds.origin.x + bounds.size.width + f64::from(offset)).ceil() as i32;
        let bottom = bounds.origin.y.floor() as i32;
        let top = (bounds.origin.y + bounds.size.height).ceil() as i32;
        let width = (right - left + 2 * PADDING) as u32;
        let height = (top - bottom + 2 * PADDING) as u32;
        let coverage = draw_glyph(
            &device_font,
            glyph,
            width,
            height,
            CGPoint {
                x: f64::from(PADDING - left) + f64::from(offset),
                y: f64::from(PADDING - bottom),
            },
        );
        let (x, y) = self.allocate(width, height)?;
        for row in 0..height {
            let target = ((y + row) * ATLAS_SIDE + x) as usize;
            let source = (row * width) as usize;
            self.pixels[target..target + width as usize].copy_from_slice(&coverage[source..source + width as usize]);
        }
        self.revision += 1;
        Some(AtlasEntry {
            x,
            y,
            width,
            height,
            left: left - PADDING,
            top: -(top + PADDING),
        })
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width > ATLAS_SIDE || height > ATLAS_SIDE {
            return None;
        }
        if let Some(shelf) = self.shelves.iter_mut().find(|shelf| {
            shelf.height >= height && shelf.height <= height + height / 4 + 2 && shelf.cursor + width <= ATLAS_SIDE
        }) {
            let position = (shelf.cursor, shelf.y);
            shelf.cursor += width;
            return Some(position);
        }
        let next_y = self.shelves.last().map_or(0, |shelf| shelf.y + shelf.height);
        if next_y + height > ATLAS_SIDE {
            self.pixels.fill(0);
            self.shelves.clear();
            self.entries.clear();
            return self.allocate(width, height);
        }
        self.shelves.push(Shelf {
            y: next_y,
            height,
            cursor: width,
        });
        Some((0, next_y))
    }
}

fn draw_glyph(font: &CTFont, glyph: u16, width: u32, height: u32, origin: CGPoint) -> Vec<u8> {
    let stride = width as usize * BYTES_PER_PIXEL;
    let mut bitmap = vec![0_u8; stride * height as usize];
    let space = CGColorSpace::new_device_rgb().expect("the device RGB space exists");
    let info = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
    let context: objc2_core_foundation::CFRetained<CGContext> = unsafe {
        CGBitmapContextCreate(
            bitmap.as_mut_ptr().cast::<c_void>(),
            width as usize,
            height as usize,
            8,
            stride,
            Some(&space),
            info,
        )
    }
    .expect("a glyph bitmap context is created");
    let context_ref = Some(&*context);
    CGContext::set_rgb_fill_color(context_ref, 0.0, 0.0, 0.0, 1.0);
    CGContext::fill_rect(
        context_ref,
        CGRect::new(
            CGPoint::ZERO,
            objc2_core_foundation::CGSize::new(f64::from(width), f64::from(height)),
        ),
    );
    CGContext::set_rgb_fill_color(context_ref, 1.0, 1.0, 1.0, 1.0);
    CGContext::set_allows_font_smoothing(context_ref, true);
    CGContext::set_should_smooth_fonts(context_ref, true);
    CGContext::set_allows_font_subpixel_positioning(context_ref, true);
    CGContext::set_should_subpixel_position_fonts(context_ref, true);
    CGContext::set_allows_font_subpixel_quantization(context_ref, false);
    CGContext::set_should_subpixel_quantize_fonts(context_ref, false);
    let mut glyph_id = glyph;
    let mut position = origin;
    unsafe {
        font.draw_glyphs(NonNull::from(&mut glyph_id), NonNull::from(&mut position), 1, &context);
    }
    drop(context);
    bitmap
        .chunks_exact(BYTES_PER_PIXEL)
        .map(|pixel| pixel[GREEN_CHANNEL])
        .collect()
}
