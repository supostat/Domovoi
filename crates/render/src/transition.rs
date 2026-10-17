use std::time::{Duration, Instant};

use domovoi_theme::Motion;

pub const HOVER_DIM_OPACITY: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub motion: Motion,
    pub started: Instant,
    pub duration: Duration,
}

impl Transition {
    pub fn start(motion: Motion, started: Instant, reduce_motion: bool) -> Self {
        Self {
            motion,
            started,
            duration: motion.duration(reduce_motion),
        }
    }

    pub fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let linear = (now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32()).min(1.0);
        ease_out(linear)
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) >= self.duration
    }
}

fn ease_out(linear: f32) -> f32 {
    1.0 - (1.0 - linear).powi(3)
}
