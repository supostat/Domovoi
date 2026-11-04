use objc2_foundation::NSPoint;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragEnd {
    pub screen_point: NSPoint,
    pub accepted: bool,
}
