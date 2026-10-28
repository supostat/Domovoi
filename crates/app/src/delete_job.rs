use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use domovoi_rules::{DeleteProgress, DeleteReport, DeleteSubject, delete};
use domovoi_scanner::CancelToken;

#[derive(Debug)]
pub struct DeleteJob {
    thread: Option<JoinHandle<DeleteReport>>,
    latest: Arc<Mutex<DeleteProgress>>,
    cancel: CancelToken,
}

impl DeleteJob {
    pub fn start(subjects: Vec<DeleteSubject>) -> Self {
        let latest = Arc::new(Mutex::new(DeleteProgress::default()));
        let cancel = CancelToken::default();
        let thread = {
            let cancel = cancel.clone();
            let latest = Arc::clone(&latest);
            thread::spawn(move || delete(subjects, &cancel, &|step| keep_furthest(&latest, step)))
        };
        Self {
            thread: Some(thread),
            latest,
            cancel,
        }
    }

    pub fn progress(&self) -> DeleteProgress {
        *self.latest.lock().expect("the progress is not poisoned")
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn finish(mut self) -> DeleteReport {
        self.thread
            .take()
            .expect("a delete finishes once")
            .join()
            .expect("the deleting thread does not panic")
    }
}

fn keep_furthest(latest: &Mutex<DeleteProgress>, step: DeleteProgress) {
    let mut latest = latest.lock().expect("the progress is not poisoned");
    if (step.files_done, step.subjects_done) >= (latest.files_done, latest.subjects_done) {
        *latest = step;
    }
}
