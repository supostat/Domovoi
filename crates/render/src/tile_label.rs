use crate::point::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileLabel {
    pub origin: Point,
    pub max_width: f32,
    pub heading: bool,
}
