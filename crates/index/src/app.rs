use crate::timestamp::Timestamp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct AppId(pub(crate) u32);

impl AppId {
    pub(crate) const NONE: Self = Self(u32::MAX);

    pub fn is_some(self) -> bool {
        self != Self::NONE
    }
}

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum Installation {
    Installed { bundle_path: String },
    System,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct App {
    pub bundle_id: String,
    pub installation: Installation,
    pub last_opened: Timestamp,
}

impl App {
    pub fn is_leftover(&self) -> bool {
        self.installation == Installation::Missing
    }
}
