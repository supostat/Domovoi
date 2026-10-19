use std::thread::{self, JoinHandle};

use domovoi_index::DirectoryListing;
use domovoi_scanner::{ScanError, ScanOptions};

use crate::pending_delta::PendingDelta;

type DeltaReading = (PendingDelta, Result<Vec<DirectoryListing>, ScanError>);

#[derive(Debug)]
pub(crate) struct DeltaJob {
    thread: Option<JoinHandle<DeltaReading>>,
    pub covered_events: usize,
}

impl DeltaJob {
    pub(crate) fn start(pending: PendingDelta, covered_events: usize) -> Self {
        let thread = thread::spawn(move || {
            let listings = pending.read(&ScanOptions::default());
            (pending, listings)
        });
        Self {
            thread: Some(thread),
            covered_events,
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub(crate) fn finish(mut self) -> DeltaReading {
        self.thread
            .take()
            .expect("a delta reading finishes once")
            .join()
            .expect("the delta reading thread does not panic")
    }
}
