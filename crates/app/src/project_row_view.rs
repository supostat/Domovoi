use objc2::rc::Retained;
use objc2_app_kit::NSTextField;

use crate::check_box::CheckBox;

#[derive(Debug)]
pub struct ProjectRowView {
    pub check: Retained<CheckBox>,
    pub name: Retained<NSTextField>,
    pub path: Retained<NSTextField>,
    pub artifacts: Retained<NSTextField>,
    pub age: Retained<NSTextField>,
    pub size: Retained<NSTextField>,
}
