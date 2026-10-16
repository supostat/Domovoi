#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Badges(u8);

impl Badges {
    pub const HARD_LINK: Self = Self(1);
    pub const CLONE: Self = Self(1 << 1);
    pub const CLONE_CANONICAL: Self = Self(1 << 2);
    pub const COMPRESSED: Self = Self(1 << 3);

    pub fn contains(self, badge: Self) -> bool {
        self.0 & badge.0 == badge.0
    }

    pub(crate) fn insert(&mut self, badge: Self) {
        self.0 |= badge.0;
    }

    pub(crate) fn remove(&mut self, badge: Self) {
        self.0 &= !badge.0;
    }
}
