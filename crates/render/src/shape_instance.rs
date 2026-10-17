use bytemuck::{Pod, Zeroable};

pub const SHAPE_ARC: u32 = 0;
pub const SHAPE_RECT: u32 = 1;
pub const SHAPE_DISC: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShapeInstance {
    pub geometry: [f32; 4],
    pub previous: [f32; 4],
    pub fill: [f32; 4],
    pub stroke: [f32; 4],
    pub center: [f32; 2],
    pub corner: f32,
    pub stroke_width: f32,
    pub kind: u32,
    pub dashed: u32,
    pub padding: [u32; 2],
}
