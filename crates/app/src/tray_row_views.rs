use objc2::rc::Retained;
use objc2_app_kit::NSTextField;

use crate::shell_button::ShellButton;

#[derive(Debug)]
pub struct TrayRowViews {
    pub title: Retained<NSTextField>,
    pub size: Retained<NSTextField>,
    pub unmark: Option<Retained<ShellButton>>,
}
