use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    ProtocolMismatch,
    ModelNotLoaded,
    LoadFailed,
    LoadCancelled,
    InvalidGrammar,
    OutOfMemory,
    Busy,
    Stalled,
    Unavailable,
    Internal,
}

impl ErrorCode {
    pub const ALL: [Self; 10] = [
        Self::ProtocolMismatch,
        Self::ModelNotLoaded,
        Self::LoadFailed,
        Self::LoadCancelled,
        Self::InvalidGrammar,
        Self::OutOfMemory,
        Self::Busy,
        Self::Stalled,
        Self::Unavailable,
        Self::Internal,
    ];

    pub fn name(self) -> &'static CStr {
        match self {
            Self::ProtocolMismatch => c"protocol_mismatch",
            Self::ModelNotLoaded => c"model_not_loaded",
            Self::LoadFailed => c"load_failed",
            Self::LoadCancelled => c"load_cancelled",
            Self::InvalidGrammar => c"invalid_grammar",
            Self::OutOfMemory => c"out_of_memory",
            Self::Busy => c"busy",
            Self::Stalled => c"stalled",
            Self::Unavailable => c"unavailable",
            Self::Internal => c"internal",
        }
    }

    pub fn from_name(name: &CStr) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.name() == name)
    }
}
