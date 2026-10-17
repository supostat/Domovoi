use std::cell::RefCell;

use objc2::rc::Retained;
use objc2_foundation::NSString;

use crate::duplicates_layout::DuplicatesLayout;
use crate::group_model::GroupModel;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;

#[derive(Debug)]
pub struct DuplicatesSourceModel {
    pub groups: Vec<GroupModel>,
    pub layout: DuplicatesLayout,
    pub clone_mode: bool,
    pub palette: ShellPalette,
    pub width: f64,
    pub actions: Retained<ShellActions>,
    pub keys: RefCell<Vec<Retained<NSString>>>,
}
