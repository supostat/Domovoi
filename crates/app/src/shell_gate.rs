use crate::shell_action::ShellAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellGate {
    pub session_open: bool,
    pub tray_open: bool,
    pub searching: bool,
}

impl ShellGate {
    pub fn allows(self, action: ShellAction) -> bool {
        match action {
            ShellAction::ToggleDuplicateSearch => self.searching || self.session_open,
            ShellAction::Rescan => self.session_open && !self.tray_open,
            _ => !action.needs_session() || self.session_open,
        }
    }
}
