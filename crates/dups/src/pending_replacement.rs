use std::path::PathBuf;

use domovoi_rules::TrashSubject;

pub const TEMPORARY_PREFIX: &str = ".domovoi-tmp-";

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct PendingReplacement {
    pub copy: String,
    pub copy_physical: String,
    pub temporary: String,
    pub link_id: u64,
    pub alloc_size: u64,
    pub flags: u32,
    pub clone_id: u64,
}

impl PendingReplacement {
    pub(crate) fn old_body(&self) -> TrashSubject {
        TrashSubject {
            original: PathBuf::from(&self.copy),
            original_physical: PathBuf::from(&self.copy_physical),
            location: PathBuf::from(&self.temporary),
            link_id: self.link_id,
            alloc_size: self.alloc_size,
            flags: self.flags,
            clone_id: self.clone_id,
            rule_id: None,
        }
    }
}
