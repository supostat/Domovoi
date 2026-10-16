use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    InCloud,
    TimeLimit,
    Changed,
    Vanished,
    Unreadable(i32),
}

impl SkipReason {
    pub(crate) fn of(error: &io::Error) -> Self {
        match error.raw_os_error() {
            Some(libc::EDEADLK) => Self::InCloud,
            Some(libc::ENOENT) => Self::Vanished,
            Some(code) => Self::Unreadable(code),
            None => Self::Changed,
        }
    }
}
