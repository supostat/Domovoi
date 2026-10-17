#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Access,
    Scanning,
    Main,
    Recommendations,
    Duplicates,
    Tray,
}

impl Screen {
    pub const ALL: [Self; 6] = [
        Self::Access,
        Self::Scanning,
        Self::Main,
        Self::Recommendations,
        Self::Duplicates,
        Self::Tray,
    ];

    pub fn number(self) -> &'static str {
        match self {
            Self::Access => "01",
            Self::Scanning => "02",
            Self::Main => "03",
            Self::Recommendations => "04",
            Self::Duplicates => "06",
            Self::Tray => "08",
        }
    }

    pub fn from_number(number: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|screen| screen.number() == number)
    }
}
