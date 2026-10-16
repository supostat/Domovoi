#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Llama,
    Fake,
}

impl BackendKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "llama" => Some(Self::Llama),
            "fake" => Some(Self::Fake),
            _ => None,
        }
    }
}
