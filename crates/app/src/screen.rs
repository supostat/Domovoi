#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Access,
    Scanning,
    Main,
}

impl Screen {
    pub const ALL: [Self; 3] = [Self::Access, Self::Scanning, Self::Main];

    pub fn number(self) -> &'static str {
        match self {
            Self::Access => "01",
            Self::Scanning => "02",
            Self::Main => "03",
        }
    }

    pub fn from_number(number: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|screen| screen.number() == number)
    }
}
