use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct FrameUniforms {
    pub(crate) viewport: [f32; 2],
    pub(crate) scale: f32,
    pub(crate) progress: f32,
}
