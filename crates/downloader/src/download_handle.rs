use std::sync::Arc;
use std::thread::JoinHandle;

use crate::download_control::DownloadControl;
use crate::download_outcome::DownloadOutcome;
use crate::stop_request::StopRequest;

#[derive(Debug)]
pub struct DownloadHandle {
    pub(crate) control: Arc<DownloadControl>,
    pub(crate) worker: JoinHandle<DownloadOutcome>,
}

impl DownloadHandle {
    pub fn pause(&self) {
        self.control.request(StopRequest::Pause);
    }

    pub fn cancel(&self) {
        self.control.request(StopRequest::Cancel);
    }

    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }

    pub fn wait(self) -> DownloadOutcome {
        self.worker.join().expect("the download worker does not panic")
    }
}
