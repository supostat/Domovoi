use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use domovoi_apfs_sys::container_capacity;
use domovoi_scanner::CancelToken;

use crate::purge_finish::PurgeFinish;
use crate::purge_progress::PurgeProgress;
use crate::purge_report::PurgeReport;
use crate::recovery_request::RecoveryRequest;
use crate::space_recovering::SpaceRecovering;

pub const PURGE_TIMEOUT: Duration = Duration::from_secs(120);
pub const PURGE_STALL: Duration = Duration::from_secs(5);
const AVAILABLE_POLL: Duration = Duration::from_millis(500);
const REFUSED_WITHOUT_REASON: &str = "CoreServices did not finish the recovery";

#[derive(Debug)]
pub struct PurgeJob {
    thread: Option<JoinHandle<io::Result<PurgeReport>>>,
    progress: Arc<Mutex<PurgeProgress>>,
    cancel: CancelToken,
}

impl PurgeJob {
    pub fn start<R, A>(recovering: R, volume: PathBuf, available: A, timeout: Duration) -> Self
    where
        R: SpaceRecovering + Send + 'static,
        A: Fn(&Path) -> io::Result<u64> + Send + 'static,
    {
        let progress = Arc::new(Mutex::new(PurgeProgress::default()));
        let cancel = CancelToken::default();
        let thread = {
            let progress = Arc::clone(&progress);
            let cancel = cancel.clone();
            thread::spawn(move || {
                Release {
                    mechanism: &recovering,
                    volume: &volume,
                    available: &available,
                    timeout,
                    progress: &progress,
                    cancel: &cancel,
                }
                .run()
            })
        };
        Self {
            thread: Some(thread),
            progress,
            cancel,
        }
    }

    pub fn progress(&self) -> PurgeProgress {
        *self.progress.lock().expect("the progress is not poisoned")
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn finish(mut self) -> io::Result<PurgeReport> {
        self.thread
            .take()
            .expect("a release finishes once")
            .join()
            .expect("the releasing thread does not panic")
    }
}

pub(crate) fn available_capacity(volume: &Path) -> io::Result<u64> {
    container_capacity(volume).map(|capacity| capacity.available)
}

struct Release<'a, R, A> {
    mechanism: &'a R,
    volume: &'a Path,
    available: &'a A,
    timeout: Duration,
    progress: &'a Mutex<PurgeProgress>,
    cancel: &'a CancelToken,
}

impl<R, A> Release<'_, R, A>
where
    R: SpaceRecovering,
    A: Fn(&Path) -> io::Result<u64>,
{
    fn run(&self) -> io::Result<PurgeReport> {
        let estimate = self.mechanism.estimate(self.volume)?;
        let available_before = (self.available)(self.volume)?;
        self.publish(PurgeProgress { estimate, freed: 0 });
        let mut request = self.mechanism.start(self.volume, estimate)?;
        let finish = match self.watch(&mut request, estimate, available_before) {
            Ok(finish) => finish,
            Err(error) => {
                self.mechanism.cancel(&request);
                return Err(error);
            }
        };
        let available_after = (self.available)(self.volume)?;
        Ok(PurgeReport {
            estimate,
            freed: available_after.saturating_sub(available_before),
            available_before,
            available_after,
            finish,
        })
    }

    fn watch(&self, request: &mut R::Request, estimate: u64, available_before: u64) -> io::Result<PurgeFinish> {
        let started = Instant::now();
        let mut peak = available_before;
        let mut last_growth = started;
        loop {
            if let Some(outcome) = request.outcome() {
                return if outcome.succeeded {
                    Ok(PurgeFinish::Completed)
                } else {
                    Err(io::Error::other(
                        outcome.error.unwrap_or_else(|| REFUSED_WITHOUT_REASON.to_owned()),
                    ))
                };
            }
            if self.cancel.is_cancelled() {
                self.mechanism.cancel(request);
                return Ok(PurgeFinish::Cancelled);
            }
            if started.elapsed() >= self.timeout {
                self.mechanism.cancel(request);
                return Ok(PurgeFinish::TimedOut);
            }
            let available = (self.available)(self.volume)?;
            if available > peak {
                peak = available;
                last_growth = Instant::now();
            }
            let freed = available.saturating_sub(available_before);
            self.publish(PurgeProgress { estimate, freed });
            if freed >= estimate && last_growth.elapsed() >= PURGE_STALL {
                return Ok(PurgeFinish::Stalled);
            }
            thread::sleep(AVAILABLE_POLL);
        }
    }

    fn publish(&self, progress: PurgeProgress) {
        *self.progress.lock().expect("the progress is not poisoned") = progress;
    }
}
