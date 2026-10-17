use crate::rect::Rect;

#[derive(Clone, Debug, PartialEq)]
pub struct AccessibleSegment {
    pub node: usize,
    pub label: String,
    pub frame: Rect,
    pub is_center: bool,
}
