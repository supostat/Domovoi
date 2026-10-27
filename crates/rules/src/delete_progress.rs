#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeleteProgress {
    pub done: usize,
    pub total: usize,
    pub subjects_done: usize,
}
