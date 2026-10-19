use std::path::PathBuf;

use domovoi_index::Reclaimable;
use domovoi_scanner::ScanError;
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::measure_progress::MeasureProgress;
use crate::tray_measure_job::TrayMeasureJob;

#[derive(Debug)]
pub(crate) struct TrayMeasurement {
    pub items: Vec<PathBuf>,
    job: Option<TrayMeasureJob>,
    timer: Option<Retained<NSTimer>>,
    progress: MeasureProgress,
    estimate: Vec<Reclaimable>,
    exact: Option<Vec<Reclaimable>>,
}

impl TrayMeasurement {
    pub(crate) fn start(items: Vec<PathBuf>, estimate: Vec<Reclaimable>, timer: Retained<NSTimer>) -> Self {
        Self {
            job: Some(TrayMeasureJob::start(items.clone())),
            items,
            timer: Some(timer),
            progress: MeasureProgress::default(),
            estimate,
            exact: None,
        }
    }

    pub(crate) fn advance(&mut self) -> Result<(), ScanError> {
        let Some(job) = self.job.as_mut() else {
            return Ok(());
        };
        self.progress = job.progress();
        if !job.is_finished() {
            return Ok(());
        }
        self.stop_timer();
        let finished = self.job.take().expect("a running measurement has a job");
        self.exact = Some(finished.finish()?.sizes);
        Ok(())
    }

    pub(crate) fn progress(&self) -> MeasureProgress {
        self.progress
    }

    pub(crate) fn exact(&self) -> Option<&[Reclaimable]> {
        self.exact.as_deref()
    }

    pub(crate) fn estimate(&self) -> &[Reclaimable] {
        &self.estimate
    }

    fn stop_timer(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
    }
}

impl Drop for TrayMeasurement {
    fn drop(&mut self) {
        self.stop_timer();
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
}
