use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_apfs_sys::data_volume;
use domovoi_app::{PURGE_STALL, PurgeFinish, PurgeJob, PurgeProgress, PurgeReport, RecoveryRequest, SpaceRecovering};
use domovoi_index::{RecoveryOutcome, SpaceRecovery};
use domovoi_testkit::test_timeout;

const ESTIMATE: u64 = 1_000;
const BEFORE: u64 = 5_000;
const POLL: Duration = Duration::from_millis(20);
const PATIENT: Duration = Duration::from_secs(60);
const IMPATIENT: Duration = Duration::from_millis(1);

struct FakeMechanism {
    available: Arc<AtomicU64>,
    available_after_start: u64,
    completion: Option<RecoveryOutcome>,
    requested: Arc<AtomicU64>,
    cancelled: Arc<AtomicBool>,
}

struct FakeRequest {
    completion: Option<RecoveryOutcome>,
}

impl RecoveryRequest for FakeRequest {
    fn outcome(&mut self) -> Option<RecoveryOutcome> {
        self.completion.clone()
    }
}

impl SpaceRecovering for FakeMechanism {
    type Request = FakeRequest;

    fn estimate(&self, _volume: &Path) -> io::Result<u64> {
        Ok(ESTIMATE)
    }

    fn start(&self, _volume: &Path, bytes: u64) -> io::Result<FakeRequest> {
        self.requested.store(bytes, Ordering::SeqCst);
        self.available.store(self.available_after_start, Ordering::SeqCst);
        Ok(FakeRequest {
            completion: self.completion.clone(),
        })
    }

    fn cancel(&self, _request: &FakeRequest) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

struct Observed {
    requested: Arc<AtomicU64>,
    cancelled: Arc<AtomicBool>,
}

impl Observed {
    fn requested(&self) -> u64 {
        self.requested.load(Ordering::SeqCst)
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

fn completed(available: u64) -> RecoveryOutcome {
    RecoveryOutcome {
        succeeded: true,
        available,
        error: None,
    }
}

fn start(available_after_start: u64, completion: Option<RecoveryOutcome>, timeout: Duration) -> (PurgeJob, Observed) {
    let available = Arc::new(AtomicU64::new(BEFORE));
    let observed = Observed {
        requested: Arc::new(AtomicU64::new(0)),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let mechanism = FakeMechanism {
        available: Arc::clone(&available),
        available_after_start,
        completion,
        requested: Arc::clone(&observed.requested),
        cancelled: Arc::clone(&observed.cancelled),
    };
    let job = PurgeJob::start(
        mechanism,
        PathBuf::from("/fake/volume"),
        move |_: &Path| Ok(available.load(Ordering::SeqCst)),
        timeout,
    );
    (job, observed)
}

fn wait(job: PurgeJob) -> (PurgeReport, Vec<PurgeProgress>) {
    let deadline = Instant::now() + test_timeout();
    let mut seen = Vec::new();
    while !job.is_finished() {
        assert!(Instant::now() < deadline, "the release did not finish in time");
        seen.push(job.progress());
        thread::sleep(POLL);
    }
    seen.push(job.progress());
    (job.finish().expect("the release reports"), seen)
}

#[test]
fn the_completion_callback_finishes_the_release_and_the_growth_is_what_was_freed() {
    let (job, observed) = start(BEFORE + ESTIMATE, Some(completed(BEFORE + ESTIMATE)), PATIENT);
    let (report, _) = wait(job);
    assert_eq!(
        report,
        PurgeReport {
            estimate: ESTIMATE,
            freed: ESTIMATE,
            available_before: BEFORE,
            available_after: BEFORE + ESTIMATE,
            finish: PurgeFinish::Completed,
        }
    );
    assert_eq!(
        observed.requested(),
        ESTIMATE,
        "the mechanism is asked for the estimate"
    );
    assert!(!observed.cancelled(), "a completed release is not cancelled");
}

#[test]
fn a_release_without_a_callback_finishes_once_the_space_stops_growing() {
    let started = Instant::now();
    let (job, observed) = start(BEFORE + ESTIMATE, None, PATIENT);
    let (report, seen) = wait(job);
    assert_eq!(report.finish, PurgeFinish::Stalled);
    assert_eq!(report.freed, ESTIMATE);
    assert!(started.elapsed() >= PURGE_STALL, "the stall is given its time");
    assert!(
        seen.contains(&PurgeProgress {
            estimate: ESTIMATE,
            freed: ESTIMATE,
        }),
        "the progress shows the growth: {seen:?}"
    );
    assert!(!observed.cancelled(), "a stalled release is left to macOS");
}

#[test]
fn the_time_limit_sends_a_cancel_and_reports_what_was_measured() {
    let (job, observed) = start(BEFORE + 500, None, IMPATIENT);
    let (report, _) = wait(job);
    assert_eq!(report.finish, PurgeFinish::TimedOut);
    assert_eq!(report.freed, 500);
    assert!(observed.cancelled(), "the time limit cancels the request");
}

#[test]
fn a_cancel_stops_the_release_and_reports_what_was_measured() {
    let (job, observed) = start(BEFORE + 300, None, PATIENT);
    job.cancel();
    let (report, _) = wait(job);
    assert_eq!(report.finish, PurgeFinish::Cancelled);
    assert_eq!(report.freed, 300);
    assert!(observed.cancelled(), "the cancel reaches the mechanism");
}

#[test]
fn a_drop_in_the_available_space_counts_as_nothing_freed() {
    let (job, _) = start(BEFORE - 1_000, Some(completed(BEFORE - 1_000)), PATIENT);
    let (report, _) = wait(job);
    assert_eq!(report.finish, PurgeFinish::Completed);
    assert_eq!(report.freed, 0);
    assert_eq!(report.available_after, BEFORE - 1_000);
}

#[test]
fn the_recovery_functions_of_this_macos_fit_the_mechanism() {
    let recovery = SpaceRecovery::load().expect("this macOS offers the disk-space recovery functions");
    let data = data_volume().expect("data volume is mounted");
    let estimate = SpaceRecovering::estimate(&recovery, &data);
    assert!(
        estimate.is_ok(),
        "the mechanism estimates the data volume: {estimate:?}"
    );
}
