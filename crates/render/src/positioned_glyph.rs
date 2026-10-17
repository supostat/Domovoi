use objc2_core_foundation::CFRetained;
use objc2_core_text::CTFont;

#[derive(Debug)]
pub struct PositionedGlyph {
    pub font: CFRetained<CTFont>,
    pub glyph: u16,
    pub x: f32,
    pub y: f32,
}
