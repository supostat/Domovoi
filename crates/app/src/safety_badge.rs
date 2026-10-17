use domovoi_index::SafetyLevel;
use domovoi_theme::{Palette, Rgba};
use serde::Deserialize;

use crate::strings::{LEVEL_CHECK_YOURSELF, LEVEL_RECOVERABLE, LEVEL_SAFE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyBadge {
    Safe,
    Recoverable,
    CheckYourself,
}

impl SafetyBadge {
    pub fn title(self) -> &'static str {
        match self {
            Self::Safe => LEVEL_SAFE,
            Self::Recoverable => LEVEL_RECOVERABLE,
            Self::CheckYourself => LEVEL_CHECK_YOURSELF,
        }
    }

    pub fn colors(self, palette: &Palette) -> (Rgba, Rgba) {
        match self {
            Self::Safe => (palette.ok, palette.ok_soft),
            Self::Recoverable => (palette.warn, palette.warn_soft),
            Self::CheckYourself => (palette.risk, palette.risk_soft),
        }
    }
}

impl From<SafetyLevel> for SafetyBadge {
    fn from(level: SafetyLevel) -> Self {
        match level {
            SafetyLevel::Safe => Self::Safe,
            SafetyLevel::Recoverable => Self::Recoverable,
            SafetyLevel::CheckYourself => Self::CheckYourself,
        }
    }
}
