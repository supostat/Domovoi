use std::ptr::{self, NonNull};

use objc2_core_foundation::{
    CFArray, CFAttributedString, CFBoolean, CFDictionary, CFRange, CFRetained, CFString, CFType, CGPoint,
};
use objc2_core_text::{
    CTFont, CTLine, CTLineTruncationType, CTRun, kCTFontAttributeName, kCTForegroundColorFromContextAttributeName,
};

use crate::font_library::font_for;
use crate::positioned_glyph::PositionedGlyph;
use crate::text_style::TextStyle;

const ELLIPSIS: &str = "\u{2026}";

#[derive(Debug)]
pub struct ShapedLine {
    line: CFRetained<CTLine>,
    pub style: TextStyle,
    pub truncated: bool,
}

impl ShapedLine {
    #[must_use]
    pub fn shape(text: &str, style: TextStyle) -> Self {
        Self {
            line: line_of(text, style),
            style,
            truncated: false,
        }
    }

    pub fn width(&self) -> f32 {
        unsafe {
            self.line
                .typographic_bounds(ptr::null_mut(), ptr::null_mut(), ptr::null_mut()) as f32
        }
    }

    pub fn line(&self) -> &CTLine {
        &self.line
    }

    #[must_use]
    pub fn truncated_end(&self, max_width: f32) -> Self {
        let token = line_of(ELLIPSIS, self.style);
        let truncated = unsafe {
            self.line
                .truncated_line(f64::from(max_width), CTLineTruncationType::End, Some(&token))
        };
        match truncated {
            Some(line) => Self {
                line,
                style: self.style,
                truncated: true,
            },
            None => Self {
                line: line_of("", self.style),
                style: self.style,
                truncated: true,
            },
        }
    }

    pub fn glyphs(&self) -> Vec<PositionedGlyph> {
        let runs = unsafe { CFRetained::cast_unchecked::<CFArray<CTRun>>(self.line.glyph_runs()) };
        let mut glyphs = Vec::new();
        for run in runs.iter() {
            let count = unsafe { run.glyph_count() } as usize;
            if count == 0 {
                continue;
            }
            let attributes = unsafe { CFRetained::cast_unchecked::<CFDictionary<CFString, CTFont>>(run.attributes()) };
            let font = attributes
                .get(unsafe { kCTFontAttributeName })
                .expect("Core Text attaches a font to every run");
            let mut identifiers = vec![0_u16; count];
            let mut positions = vec![CGPoint { x: 0.0, y: 0.0 }; count];
            let range = CFRange {
                location: 0,
                length: isize::try_from(count).expect("a glyph count fits in isize"),
            };
            unsafe {
                run.glyphs(
                    range,
                    NonNull::new(identifiers.as_mut_ptr()).expect("a vector is not null"),
                );
                run.positions(
                    range,
                    NonNull::new(positions.as_mut_ptr()).expect("a vector is not null"),
                );
            }
            for (glyph, position) in identifiers.into_iter().zip(positions) {
                glyphs.push(PositionedGlyph {
                    font: font.clone(),
                    glyph,
                    x: position.x as f32,
                    y: position.y as f32,
                });
            }
        }
        glyphs
    }
}

fn line_of(text: &str, style: TextStyle) -> CFRetained<CTLine> {
    let font = font_for(style);
    let keys: [&CFString; 2] = unsafe { [kCTFontAttributeName, kCTForegroundColorFromContextAttributeName] };
    let values: [&CFType; 2] = [font.as_ref(), CFBoolean::new(true).as_ref()];
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    let string = CFString::from_str(text);
    unsafe {
        let styled_text = CFAttributedString::new(None, Some(&string), Some(attributes.as_opaque()))
            .expect("Core Foundation builds an attributed string");
        CTLine::with_attributed_string(&styled_text)
    }
}
