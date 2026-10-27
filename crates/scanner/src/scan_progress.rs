use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanProgress {
    Walking { files: u64, bytes: u64, current: PathBuf },
    Walked { files: u64, bytes: u64 },
}
