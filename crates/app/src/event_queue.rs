use std::io;

use domovoi_scanner::{ScanError, VolumeEvent};

use crate::applied_batch::AppliedBatch;
use crate::delta_job::DeltaJob;
use crate::event_effect::EventEffect;
use crate::event_request::EventRequest;
use crate::read_batch::ReadBatch;
use crate::session::Session;
use crate::session_error::SessionError;

pub const HEAVY_DELTA_NODES: usize = 50_000;

#[derive(Debug)]
pub struct EventQueue {
    heavy_nodes: usize,
    queued: Vec<VolumeEvent>,
    reading: Option<DeltaJob>,
    heavy: Option<ReadBatch>,
    descended: u64,
}

impl EventQueue {
    pub fn new(heavy_nodes: usize) -> Self {
        Self {
            heavy_nodes,
            queued: Vec::new(),
            reading: None,
            heavy: None,
            descended: 0,
        }
    }

    pub fn advance(
        &mut self,
        arrived: impl IntoIterator<Item = VolumeEvent>,
        session: &mut Session,
    ) -> Result<EventEffect, SessionError> {
        self.queued.extend(arrived);
        if self.heavy.is_some() {
            return Ok(EventEffect::Heavy);
        }
        let mut effect = EventEffect::Ignored;
        if let Some(job) = self.reading.take_if(|job| job.is_finished()) {
            let covered_events = job.covered_events;
            let (pending, listings) = job.finish();
            match listings {
                Ok(listings) => {
                    let batch = ReadBatch::new(pending, listings, covered_events);
                    if batch.nodes() >= self.heavy_nodes {
                        self.heavy = Some(batch);
                        return Ok(EventEffect::Heavy);
                    }
                    effect = self.settle(batch.apply(session))?;
                }
                Err(error) => {
                    let error = SessionError::Scan(error);
                    if !is_retried(&error) {
                        return Err(error);
                    }
                }
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

    pub fn take_heavy(&mut self) -> Option<ReadBatch> {
        self.heavy.take()
    }

    pub fn settle(&mut self, applied: AppliedBatch) -> Result<EventEffect, SessionError> {
        match applied.outcome {
            Ok(effect) => {
                self.queued.drain(..applied.covered_events);
                self.descended += applied.descended;
                Ok(effect)
            }
            Err(error) if is_retried(&error) => Ok(EventEffect::Ignored),
            Err(error) => Err(error),
        }
    }

    pub fn is_idle(&self) -> bool {
        self.queued.is_empty() && self.reading.is_none() && self.heavy.is_none()
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
