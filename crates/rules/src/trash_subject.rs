use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashSubject {
    pub original: PathBuf,
    pub original_physical: PathBuf,
    pub location: PathBuf,
    pub link_id: u64,
    pub alloc_size: u64,
    pub flags: u32,
    pub clone_id: u64,
    pub rule_id: Option<String>,
}
