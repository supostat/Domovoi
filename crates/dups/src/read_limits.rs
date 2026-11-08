use std::time::Instant;

use domovoi_scanner::CancelToken;

pub(crate) struct ReadLimits<'search> {
    pub(crate) cancel: &'search CancelToken,
    pub(crate) deadline: Option<Instant>,
}

impl ReadLimits<'_> {
    pub(crate) fn passed_deadline(&self) -> bool {
        self.deadline.is_some_and(|deadline| Instant::now() >= deadline)
    }
}
