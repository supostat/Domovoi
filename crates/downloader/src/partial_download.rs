use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialDownload {
    pub entry_id: String,
    pub path: PathBuf,
    pub received: u64,
}
