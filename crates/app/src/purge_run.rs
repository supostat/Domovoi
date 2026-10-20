use domovoi_rules::PurgeReport;
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::purge_job::PurgeJob;
use crate::purge_status::PurgeStatus;

#[derive(Debug)]
pub(crate) struct PurgeRun {
    job: Option<PurgeJob>,
    timer: Retained<NSTimer>,
}

impl PurgeRun {
    pub(crate) fn start(job: PurgeJob, timer: Retained<NSTimer>) -> Self {
        Self { job: Some(job), timer }
    }

    pub(crate) fn status(&self) -> PurgeStatus {
        let progress = self.job().progress();
        PurgeStatus {
            done: progress.done,
            total: progress.total,
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.job.as_ref().is_some_and(PurgeJob::is_finished)
    }

    pub(crate) fn stop(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }

    pub(crate) fn finish(mut self) -> PurgeReport {
        self.timer.invalidate();
        self.job.take().expect("a purge finishes once").finish()
    }

    fn job(&self) -> &PurgeJob {
        self.job.as_ref().expect("a running purge has a job")
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
