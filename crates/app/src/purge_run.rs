use std::io;

use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::purge_job::PurgeJob;
use crate::purge_progress::PurgeProgress;
use crate::purge_report::PurgeReport;

pub(crate) const PURGE_TICK: f64 = 0.25;

#[derive(Debug)]
pub(crate) struct PurgeRun {
    job: Option<PurgeJob>,
    timer: Retained<NSTimer>,
}

impl PurgeRun {
    pub(crate) fn start(job: PurgeJob, timer: Retained<NSTimer>) -> Self {
        Self { job: Some(job), timer }
    }

    pub(crate) fn progress(&self) -> PurgeProgress {
        self.job().progress()
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.job.as_ref().is_some_and(PurgeJob::is_finished)
    }

    pub(crate) fn stop(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }

    pub(crate) fn finish(mut self) -> io::Result<PurgeReport> {
        self.timer.invalidate();
        self.job.take().expect("a release finishes once").finish()
    }

    fn job(&self) -> &PurgeJob {
        self.job.as_ref().expect("a running release has a job")
    }
}

impl Drop for PurgeRun {
    fn drop(&mut self) {
        self.timer.invalidate();
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
}
