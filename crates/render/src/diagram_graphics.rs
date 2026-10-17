use crate::diagram_renderer::DiagramRenderer;
use crate::glyph_atlas::GlyphAtlas;

#[derive(Debug)]
pub struct DiagramGraphics {
    pub renderer: DiagramRenderer,
    pub atlas: GlyphAtlas,
}
