use domovoi_index::DirectoryListing;

use crate::applied_batch::AppliedBatch;
use crate::pending_delta::PendingDelta;
use crate::session::Session;

#[derive(Debug)]
pub struct ReadBatch {
    pending: PendingDelta,
    listings: Vec<DirectoryListing>,
    covered_events: usize,
}

impl ReadBatch {
    pub(crate) fn new(pending: PendingDelta, listings: Vec<DirectoryListing>, covered_events: usize) -> Self {
        Self {
            pending,
            listings,
            covered_events,
        }
    }

    pub fn nodes(&self) -> usize {
        self.listings.iter().map(DirectoryListing::nodes).sum()
    }

    pub fn apply(self, session: &mut Session) -> AppliedBatch {
        let descended = self.listings.iter().map(|listing| listing.descended).sum();
        AppliedBatch {
            covered_events: self.covered_events,
            descended,
            outcome: session.apply_listings(&self.pending, self.listings),
        }
    }
}
