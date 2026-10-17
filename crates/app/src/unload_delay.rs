use std::time::Duration;

use serde::Deserialize;

use crate::strings::{
    UNLOAD_FIFTEEN_MINUTES, UNLOAD_FIVE_MINUTES, UNLOAD_IMMEDIATELY, UNLOAD_NEVER, UNLOAD_SIXTY_MINUTES,
};

const MINUTE: u64 = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnloadDelay {
    Immediately,
    #[default]
    FiveMinutes,
    FifteenMinutes,
    SixtyMinutes,
    Never,
}

impl UnloadDelay {
    pub const ALL: [Self; 5] = [
        Self::Immediately,
        Self::FiveMinutes,
        Self::FifteenMinutes,
        Self::SixtyMinutes,
        Self::Never,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Immediately => UNLOAD_IMMEDIATELY,
            Self::FiveMinutes => UNLOAD_FIVE_MINUTES,
            Self::FifteenMinutes => UNLOAD_FIFTEEN_MINUTES,
            Self::SixtyMinutes => UNLOAD_SIXTY_MINUTES,
            Self::Never => UNLOAD_NEVER,
        }
    }

    pub fn after(self) -> Option<Duration> {
        match self {
            Self::Immediately => Some(Duration::ZERO),
            Self::FiveMinutes => Some(Duration::from_secs(5 * MINUTE)),
            Self::FifteenMinutes => Some(Duration::from_secs(15 * MINUTE)),
            Self::SixtyMinutes => Some(Duration::from_secs(60 * MINUTE)),
            Self::Never => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Immediately => "immediately",
            Self::FiveMinutes => "five_minutes",
            Self::FifteenMinutes => "fifteen_minutes",
            Self::SixtyMinutes => "sixty_minutes",
            Self::Never => "never",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|delay| delay.key() == key)
    }
}
