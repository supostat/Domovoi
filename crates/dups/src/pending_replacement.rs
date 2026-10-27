use std::path::PathBuf;

use domovoi_rules::DeleteSubject;

pub const TEMPORARY_PREFIX: &str = ".domovoi-tmp-";

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct PendingReplacement {
    pub copy: String,
    pub copy_physical: String,
    pub temporary: String,
    pub link_id: u64,
    pub alloc_size: u64,
}

impl PendingReplacement {
    pub(crate) fn old_body(&self) -> DeleteSubject {
        DeleteSubject {
            path: PathBuf::from(&self.copy),
            physical: PathBuf::from(&self.temporary),
            link_id: self.link_id,
            measured_bytes: self.alloc_size,
            rule_id: None,
        }
    }
}
