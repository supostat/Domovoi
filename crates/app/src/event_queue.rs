use std::io;

use domovoi_scanner::{ScanError, VolumeEvent};

use crate::delta_job::DeltaJob;
use crate::event_effect::EventEffect;
use crate::event_request::EventRequest;
use crate::session::Session;
use crate::session_error::SessionError;

#[derive(Debug, Default)]
pub struct EventQueue {
    queued: Vec<VolumeEvent>,
    reading: Option<DeltaJob>,
    descended: u64,
}

impl EventQueue {
    pub fn advance(
        &mut self,
        arrived: impl IntoIterator<Item = VolumeEvent>,
        session: &mut Session,
    ) -> Result<EventEffect, SessionError> {
        self.queued.extend(arrived);
        let mut effect = EventEffect::Ignored;
        if let Some(job) = self.reading.take_if(|job| job.is_finished()) {
            let covered_events = job.covered_events;
            let (pending, listings) = job.finish();
            let descended = listings
                .as_ref()
                .map_or(0, |listings| listings.iter().map(|listing| listing.descended).sum());
            match listings
                .map_err(SessionError::Scan)
                .and_then(|listings| session.apply_listings(&pending, listings))
            {
                Ok(applied) => {
                    self.queued.drain(..covered_events);
                    self.descended += descended;
                    effect = applied;
                }
                Err(error) if is_retried(&error) => {}
                Err(error) => return Err(error),
            }
        }
        if self.reading.is_none() && !self.queued.is_empty() {
            match session.delta_request(&self.queued) {
                EventRequest::RescanRequired => return Ok(EventEffect::RescanRequired),
                EventRequest::Delta(pending) => self.reading = Some(DeltaJob::start(pending, self.queued.len())),
            }
        }
        Ok(effect)
    }

    pub fn is_idle(&self) -> bool {
        self.queued.is_empty() && self.reading.is_none()
    }

    pub fn is_reading(&self) -> bool {
        self.reading.is_some()
    }

    pub fn descended(&self) -> u64 {
        self.descended
    }
}

fn is_retried(error: &SessionError) -> bool {
    match error {
        SessionError::Stale(_) => true,
        SessionError::Scan(ScanError::Root(error)) => error.kind() == io::ErrorKind::NotFound,
        SessionError::Scan(_) | SessionError::Io(_) => false,
    }
}
