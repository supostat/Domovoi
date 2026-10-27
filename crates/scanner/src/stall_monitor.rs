use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const STALL_LIMIT: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct StallMonitor {
    started: Instant,
    last_activity_millis: AtomicU64,
    last_path: Mutex<PathBuf>,
}

impl Default for StallMonitor {
    fn default() -> Self {
        Self::starting_at(Instant::now())
    }
}

impl StallMonitor {
    pub fn starting_at(started: Instant) -> Self {
        Self {
            started,
            last_activity_millis: AtomicU64::new(0),
            last_path: Mutex::new(PathBuf::new()),
        }
    }

    pub fn enter(&self, path: &Path, now: Instant) {
        *self.last_path.lock().expect("the last path is not poisoned") = path.to_path_buf();
        self.heartbeat(now);
    }

    pub fn heartbeat(&self, now: Instant) {
        let millis = u64::try_from(now.saturating_duration_since(self.started).as_millis()).unwrap_or(u64::MAX);
        self.last_activity_millis.store(millis, Ordering::Relaxed);
    }

    pub fn idle_for(&self, now: Instant) -> Duration {
        let last = self.started + Duration::from_millis(self.last_activity_millis.load(Ordering::Relaxed));
        now.saturating_duration_since(last)
    }

    pub fn is_stalled(&self, now: Instant, limit: Duration) -> bool {
        self.idle_for(now) > limit
    }

    pub fn last_path(&self) -> PathBuf {
        self.last_path.lock().expect("the last path is not poisoned").clone()
    }
}
