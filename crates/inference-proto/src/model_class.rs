use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelClass {
    Compact,
    Standard,
    Extended,
}

impl ModelClass {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Standard, Self::Extended];

    pub fn name(self) -> &'static CStr {
        match self {
            Self::Compact => c"compact",
            Self::Standard => c"standard",
            Self::Extended => c"extended",
        }
    }

    pub fn from_name(name: &CStr) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.name() == name)
    }
}
