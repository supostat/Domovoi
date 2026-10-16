use std::time::Duration;

use crate::stop_reason::StopReason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationStats {
    pub prompt_tokens: u32,
    pub generated_tokens: u32,
    pub prompt_duration: Duration,
    pub generation_duration: Duration,
    pub stop: StopReason,
}
