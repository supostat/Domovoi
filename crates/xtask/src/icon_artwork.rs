#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconArtwork {
    Full,
    Small,
}

impl IconArtwork {
    pub(crate) fn source(self) -> &'static str {
        match self {
            Self::Full => "design/icon/domovoi-icon.svg",
            Self::Small => "design/icon/domovoi-icon-small.svg",
        }
    }
}
