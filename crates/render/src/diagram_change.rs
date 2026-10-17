use objc2_foundation::NSPoint;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramChange {
    pub focus: usize,
    pub hovered: Option<usize>,
    pub pointer: Option<NSPoint>,
}
