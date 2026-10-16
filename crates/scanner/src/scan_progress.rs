use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanProgress {
    pub files: u64,
    pub bytes: u64,
    pub current: PathBuf,
}
