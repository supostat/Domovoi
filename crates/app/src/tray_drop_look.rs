use crate::drop_kind::DropKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayDropLook {
    Rest,
    Waiting,
    Add,
    Denied,
    Already,
}

impl TrayDropLook {
    pub fn of(kind: DropKind) -> Self {
        match kind {
            DropKind::Add => Self::Add,
            DropKind::Denied => Self::Denied,
            DropKind::Already => Self::Already,
        }
    }
}
