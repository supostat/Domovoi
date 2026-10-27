use crate::hit::Hit;
use crate::rect::Rect;

#[derive(Clone, Debug, PartialEq)]
pub struct AccessibleSegment {
    pub node: usize,
    pub label: String,
    pub frame: Rect,
    pub is_center: bool,
    pub is_group: bool,
}

impl AccessibleSegment {
    pub fn hit(&self) -> Hit {
        if self.is_group {
            Hit::Group(self.node)
        } else {
            Hit::Node(self.node)
        }
    }
}
