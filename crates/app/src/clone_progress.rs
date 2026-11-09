#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloneProgress {
    pub done: u64,
    pub total: u64,
}
