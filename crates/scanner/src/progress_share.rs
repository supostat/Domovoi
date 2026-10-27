use std::sync::atomic::{AtomicU64, Ordering};

pub const SHARE_UNIT: u64 = 1 << 40;

#[derive(Debug)]
pub struct ProgressShare {
    done: AtomicU64,
    listed_directories: AtomicU64,
    expected_directories: Option<u64>,
}

impl ProgressShare {
    pub fn new(expected_directories: Option<u64>) -> Self {
        Self {
            done: AtomicU64::new(0),
            listed_directories: AtomicU64::new(0),
            expected_directories: expected_directories.filter(|expected| *expected > 0),
        }
    }

    pub fn complete(&self, units: u64) {
        self.done.fetch_add(units, Ordering::Relaxed);
    }

    pub fn directory_listed(&self) {
        self.listed_directories.fetch_add(1, Ordering::Relaxed);
    }

    pub fn share(&self) -> f32 {
        let share = match self.expected_directories {
            Some(expected) => self.listed_directories.load(Ordering::Relaxed) as f64 / expected as f64,
            None => self.done.load(Ordering::Relaxed) as f64 / SHARE_UNIT as f64,
        };
        share.clamp(0.0, 1.0) as f32
    }
}
