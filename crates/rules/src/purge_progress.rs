#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurgeProgress {
    pub done: usize,
    pub total: usize,
    pub entries_done: usize,
}
