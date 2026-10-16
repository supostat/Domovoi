use std::sync::Mutex;

use objc2::rc::Retained;
use objc2_foundation::NSURLSessionDataTask;

use crate::stop_request::StopRequest;

#[derive(Debug, Default)]
pub(crate) struct DownloadControl {
    stop: Mutex<Option<StopRequest>>,
    task: Mutex<Option<Retained<NSURLSessionDataTask>>>,
}

impl DownloadControl {
    pub(crate) fn request(&self, request: StopRequest) {
        {
            let mut stop = self.stop.lock().expect("the stop request lock is never poisoned");
            if *stop != Some(StopRequest::Cancel) {
                *stop = Some(request);
            }
        }
        if let Some(task) = self.task.lock().expect("the task lock is never poisoned").as_ref() {
            task.cancel();
        }
    }

    pub(crate) fn requested(&self) -> Option<StopRequest> {
        *self.stop.lock().expect("the stop request lock is never poisoned")
    }

    pub(crate) fn attach(&self, task: &Retained<NSURLSessionDataTask>) {
        *self.task.lock().expect("the task lock is never poisoned") = Some(Retained::clone(task));
        if self.requested().is_some() {
            task.cancel();
        }
    }

    pub(crate) fn detach(&self) {
        *self.task.lock().expect("the task lock is never poisoned") = None;
    }
}
