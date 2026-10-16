use crate::duplicate_group::DuplicateGroup;
use crate::skipped_file::SkippedFile;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Duplicates {
    pub groups: Vec<DuplicateGroup>,
    pub skipped: Vec<SkippedFile>,
}
