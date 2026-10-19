use crate::event_effect::EventEffect;
use crate::session_error::SessionError;

#[derive(Debug)]
pub struct AppliedBatch {
    pub(crate) covered_events: usize,
    pub(crate) descended: u64,
    pub(crate) outcome: Result<EventEffect, SessionError>,
}
