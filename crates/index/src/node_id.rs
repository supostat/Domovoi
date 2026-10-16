#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    pub const ROOT: Self = Self(0);
    pub(crate) const NONE: Self = Self(u32::MAX);

    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) fn from_index(index: usize) -> Self {
        Self(u32::try_from(index).expect("the index holds fewer than 2^32 nodes"))
    }
}
