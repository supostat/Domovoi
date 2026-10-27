use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum ScanProgress {
    Walking {
        files: u64,
        bytes: u64,
        current: PathBuf,
        share: f32,
    },
    Walked {
        files: u64,
        bytes: u64,
        share: f32,
    },
}
