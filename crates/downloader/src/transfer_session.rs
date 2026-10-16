use std::sync::mpsc::{self, Receiver};

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_foundation::{
    NSMutableURLRequest, NSOperationQueue, NSString, NSURL, NSURLRequestCachePolicy, NSURLSession,
    NSURLSessionConfiguration, NSURLSessionDataTask,
};

use crate::install_error::InstallError;
use crate::session_delegate::SessionDelegate;
use crate::task_event::TaskEvent;
use crate::transfer_event::TransferEvent;

const REQUEST_TIMEOUT_SECONDS: f64 = 60.0;

#[derive(Debug)]
pub(crate) struct TransferSession {
    session: Retained<NSURLSession>,
    events: Receiver<TaskEvent>,
}

impl TransferSession {
    pub(crate) fn new() -> Self {
        let (sender, events) = mpsc::channel();
        let delegate = SessionDelegate::new(sender);
        let configuration = NSURLSessionConfiguration::ephemeralSessionConfiguration();
        configuration.setURLCache(None);
        configuration.setRequestCachePolicy(NSURLRequestCachePolicy::ReloadIgnoringLocalCacheData);
        configuration.setTimeoutIntervalForRequest(REQUEST_TIMEOUT_SECONDS);
        let queue = NSOperationQueue::new();
        queue.setMaxConcurrentOperationCount(1);
        let session = unsafe {
            NSURLSession::sessionWithConfiguration_delegate_delegateQueue(
                &configuration,
                Some(ProtocolObject::from_ref(&*delegate)),
                Some(&queue),
            )
        };
        Self { session, events }
    }

    pub(crate) fn start(
        &self,
        url: &str,
        headers: &[(&str, String)],
    ) -> Result<Retained<NSURLSessionDataTask>, InstallError> {
        autoreleasepool(|_| {
            let location = NSURL::URLWithString(&NSString::from_str(url))
                .ok_or_else(|| InstallError::Network(format!("{url} is not a valid URL")))?;
            let request = NSMutableURLRequest::requestWithURL(&location);
            for (name, value) in headers {
                request.setValue_forHTTPHeaderField(Some(&NSString::from_str(value)), &NSString::from_str(name));
            }
            let task = self.session.dataTaskWithRequest(&request);
            task.resume();
            Ok(task)
        })
    }

    pub(crate) fn next_event(&self, task: &NSURLSessionDataTask) -> TransferEvent {
        let identifier = task.taskIdentifier();
        loop {
            let tagged = self
                .events
                .recv()
                .expect("the session delegate lives as long as the session");
            if tagged.task == identifier {
                return tagged.event;
            }
        }
    }
}

impl Drop for TransferSession {
    fn drop(&mut self) {
        self.session.invalidateAndCancel();
    }
}
