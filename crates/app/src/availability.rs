#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Availability {
    #[default]
    Off,
    NoModel,
    Loading,
    Unloaded,
    Ready,
    Answering,
}

impl Availability {
    pub fn shows_transcript(self) -> bool {
        !matches!(self, Self::Off | Self::NoModel)
    }

    pub fn accepts_questions(self) -> bool {
        matches!(self, Self::Unloaded | Self::Ready)
    }
}
