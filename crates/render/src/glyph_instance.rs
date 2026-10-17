use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GlyphInstance {
    pub rect: [f32; 4],
    pub texture: [f32; 4],
    pub color: [f32; 4],
}
