#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeleteProgress {
    pub files_done: usize,
    pub files_total: usize,
    pub subjects_done: usize,
}
