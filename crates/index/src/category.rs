#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum Category {
    Apps,
    Dev,
    Media,
    Docs,
    Cache,
    System,
    Hidden,
    Model,
    Home,
}

impl Category {
    pub const ALL: [Self; 9] = [
        Self::Apps,
        Self::Dev,
        Self::Media,
        Self::Docs,
        Self::Cache,
        Self::System,
        Self::Hidden,
        Self::Model,
        Self::Home,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Apps => "apps",
            Self::Dev => "dev",
            Self::Media => "media",
            Self::Docs => "docs",
            Self::Cache => "cache",
            Self::System => "system",
            Self::Hidden => "hidden",
            Self::Model => "model",
            Self::Home => "home",
        }
    }
}
