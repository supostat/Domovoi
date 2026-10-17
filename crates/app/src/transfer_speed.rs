use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(5);
const SHORTEST_SPAN: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
pub(crate) struct TransferSpeed {
    samples: VecDeque<(Instant, u64)>,
}

impl TransferSpeed {
    pub(crate) fn record(&mut self, at: Instant, received: u64) {
        self.samples.push_back((at, received));
        while self.samples.len() > 2
            && self
                .samples
                .front()
                .is_some_and(|(sampled, _)| at.duration_since(*sampled) > WINDOW)
        {
            self.samples.pop_front();
        }
    }

    pub(crate) fn bytes_per_second(&self) -> u64 {
        let (Some((first_at, first)), Some((last_at, last))) = (self.samples.front(), self.samples.back()) else {
            return 0;
        };
        let span = last_at.duration_since(*first_at);
        if span < SHORTEST_SPAN {
            return 0;
        }
        (last.saturating_sub(*first) as f64 / span.as_secs_f64()) as u64
    }
}
