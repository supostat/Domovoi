#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentFocus {
    position: Option<usize>,
}

impl SegmentFocus {
    pub fn position(&self) -> Option<usize> {
        self.position
    }

    pub fn step(&mut self, forward: bool, count: usize) -> Option<usize> {
        if count == 0 {
            self.position = None;
            return None;
        }
        self.position = Some(match (self.position, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(position), true) => (position + 1) % count,
            (Some(position), false) => (position + count - 1) % count,
        });
        self.position
    }

    pub fn reset(&mut self) {
        self.position = None;
    }
}
