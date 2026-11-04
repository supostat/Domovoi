use objc2::rc::Retained;
use objc2_app_kit::NSPasteboardType;
use objc2_foundation::NSString;

const NODE_TYPE: &str = "com.domovoi.node";

pub(crate) fn node_type() -> Retained<NSPasteboardType> {
    NSString::from_str(NODE_TYPE)
}
