use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneReplaced {
    pub trashed: PathBuf,
}
