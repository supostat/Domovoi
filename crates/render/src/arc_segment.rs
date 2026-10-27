use crate::hit::Hit;
use crate::segment_kind::SegmentKind;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcSegment {
    pub node: usize,
    pub kind: SegmentKind,
    pub ring: u32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub pad: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
}

impl ArcSegment {
    pub fn hit(&self) -> Hit {
        match self.kind {
            SegmentKind::Node => Hit::Node(self.node),
            SegmentKind::SmallGroup { .. } => Hit::Group(self.node),
        }
    }

    pub fn drawn_angles(&self) -> (f32, f32) {
        (self.start_angle + self.pad, self.end_angle - self.pad)
    }

    pub fn middle(&self) -> (f32, f32) {
        (
            f32::midpoint(self.start_angle, self.end_angle),
            f32::midpoint(self.inner_radius, self.outer_radius),
        )
    }
}
