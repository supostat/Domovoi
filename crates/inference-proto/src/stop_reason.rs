use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    EndOfGeneration,
    TokenLimit,
    Cancelled,
}

impl StopReason {
    pub const ALL: [Self; 3] = [Self::EndOfGeneration, Self::TokenLimit, Self::Cancelled];

    pub fn name(self) -> &'static CStr {
        match self {
            Self::EndOfGeneration => c"end_of_generation",
            Self::TokenLimit => c"token_limit",
            Self::Cancelled => c"cancelled",
        }
    }

    pub fn from_name(name: &CStr) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.name() == name)
    }
}
