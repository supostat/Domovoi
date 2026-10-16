use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    LevelZoom,
    HoverDim,
    TrashCollapse,
    CleanupOrnament,
}

impl Motion {
    pub const fn duration(self, reduce_motion: bool) -> Duration {
        if reduce_motion {
            return Duration::ZERO;
        }
        match self {
            Self::LevelZoom => Duration::from_millis(220),
            Self::HoverDim => Duration::from_millis(120),
            Self::TrashCollapse => Duration::from_millis(320),
            Self::CleanupOrnament => Duration::from_millis(1400),
        }
    }
}
