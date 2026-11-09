use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::clone_job::CloneJob;
use crate::clones_outcome::ClonesOutcome;
use crate::duplicates_state::DuplicatesState;
use crate::session::Session;
use crate::session_error::SessionError;

#[derive(Debug)]
pub(crate) struct CloneRun {
    job: Option<CloneJob>,
    timer: Retained<NSTimer>,
}

impl CloneRun {
    pub(crate) fn start(job: CloneJob, timer: Retained<NSTimer>) -> Self {
        Self { job: Some(job), timer }
    }

    pub(crate) fn absorb_progress(&mut self) -> bool {
        self.job
            .as_mut()
            .expect("a running replacement has a job")
            .absorb_progress()
    }

    pub(crate) fn state(&self) -> DuplicatesState {
        let job = self.job();
        DuplicatesState::Cloning {
            done: job.done,
            total: job.total,
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.job.as_ref().is_some_and(CloneJob::is_finished)
    }

    pub(crate) fn stop(&self) {
        if let Some(job) = &self.job {
            job.stop();
        }
    }

    pub(crate) fn finish(mut self) -> (Session, Result<ClonesOutcome, SessionError>) {
        self.timer.invalidate();
        self.job.take().expect("a replacement finishes once").finish()
    }

    fn job(&self) -> &CloneJob {
        self.job.as_ref().expect("a running replacement has a job")
    }
}

impl Drop for CloneRun {
    fn drop(&mut self) {
        self.timer.invalidate();
        if let Some(job) = &self.job {
            job.stop();
        }
    }
}
