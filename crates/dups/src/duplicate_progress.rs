use crate::duplicate_stage::DuplicateStage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuplicateProgress {
    pub stage: DuplicateStage,
    pub files_done: u64,
    pub files_total: u64,
}
