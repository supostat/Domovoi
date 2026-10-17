use crate::glyph_instance::GlyphInstance;
use crate::shape_instance::ShapeInstance;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub progress: f32,
    pub clear: [f32; 4],
    pub shapes: Vec<ShapeInstance>,
    pub glyphs: Vec<GlyphInstance>,
}
