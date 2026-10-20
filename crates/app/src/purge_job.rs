use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use domovoi_rules::{JournalEntry, PurgeProgress, PurgeReport, purge};
use domovoi_scanner::CancelToken;

#[derive(Debug)]
pub struct PurgeJob {
    thread: Option<JoinHandle<PurgeReport>>,
    latest: Arc<Mutex<PurgeProgress>>,
    cancel: CancelToken,
}

impl PurgeJob {
    pub fn start(entries: Vec<JournalEntry>) -> Self {
        let latest = Arc::new(Mutex::new(PurgeProgress::default()));
        let cancel = CancelToken::default();
        let thread = {
            let cancel = cancel.clone();
            let latest = Arc::clone(&latest);
            thread::spawn(move || purge(entries, &cancel, &|step| keep_furthest(&latest, step)))
        };
        Self {
            thread: Some(thread),
            latest,
            cancel,
        }
    }

    pub fn progress(&self) -> PurgeProgress {
        *self.latest.lock().expect("the progress is not poisoned")
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn finish(mut self) -> PurgeReport {
        self.thread
            .take()
            .expect("a purge finishes once")
            .join()
            .expect("the purging thread does not panic")
    }
}

fn keep_furthest(latest: &Mutex<PurgeProgress>, step: PurgeProgress) {
    let mut latest = latest.lock().expect("the progress is not poisoned");
    if (step.done, step.entries_done) >= (latest.done, latest.entries_done) {
        *latest = step;
    }
}
