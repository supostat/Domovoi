use crate::copy_groups::InodeKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeldKey {
    HardLinks { device: u32, file: u64 },
    Clones { device: u32, stream: u64 },
}

impl HeldKey {
    pub(crate) fn hard_links((device, file): InodeKey) -> Self {
        Self::HardLinks { device, file }
    }

    pub(crate) fn clones((device, stream): InodeKey) -> Self {
        Self::Clones { device, stream }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldGroup {
    pub key: HeldKey,
    pub bytes: u64,
    pub members_inside: u32,
    pub members_total: u32,
}
