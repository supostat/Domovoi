use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteSubject {
    pub path: PathBuf,
    pub physical: PathBuf,
    pub link_id: u64,
    pub measured_bytes: u64,
    pub rule_id: Option<String>,
}
