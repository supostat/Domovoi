use objc2::rc::Retained;
use objc2_app_kit::NSWindow;

use crate::tray_screen::TrayScreen;

#[derive(Debug)]
pub(crate) struct TraySheet {
    pub window: Retained<NSWindow>,
    pub screen: TrayScreen,
}
