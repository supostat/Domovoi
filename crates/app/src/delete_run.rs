use domovoi_rules::{DeleteRefusal, DeleteReport};
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::delete_job::DeleteJob;
use crate::delete_outcome::DeleteOutcome;
use crate::delete_status::DeleteStatus;

#[derive(Debug)]
pub(crate) struct DeleteRun {
    job: Option<DeleteJob>,
    refusals: Vec<DeleteRefusal>,
    freed_together: u64,
    timer: Retained<NSTimer>,
}

impl DeleteRun {
    pub(crate) fn start(
        job: DeleteJob,
        refusals: Vec<DeleteRefusal>,
        freed_together: u64,
        timer: Retained<NSTimer>,
    ) -> Self {
        Self {
            job: Some(job),
            refusals,
            freed_together,
            timer,
        }
    }

    pub(crate) fn status(&self) -> DeleteStatus {
        self.job().progress().into()
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.job.as_ref().is_some_and(DeleteJob::is_finished)
    }

    pub(crate) fn stop(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }

    pub(crate) fn finish(mut self) -> (DeleteReport, DeleteOutcome) {
        self.timer.invalidate();
        let report = self.job.take().expect("a delete finishes once").finish();
        let outcome = DeleteOutcome::from_report(&report, &self.refusals, self.freed_together);
        (report, outcome)
    }

    fn job(&self) -> &DeleteJob {
        self.job.as_ref().expect("a running delete has a job")
    }
}

impl Drop for DeleteRun {
    fn drop(&mut self) {
        self.timer.invalidate();
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
}
