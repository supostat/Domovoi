use crate::shell_action::ShellAction;

pub trait ActionHandler {
    fn perform(&self, action: ShellAction);

    fn allows(&self, action: ShellAction) -> bool;
}
