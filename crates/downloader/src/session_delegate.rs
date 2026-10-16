use std::sync::mpsc::Sender;

use block2::DynBlock;
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{
    NSData, NSError, NSHTTPURLResponse, NSObject, NSString, NSURLRequest, NSURLResponse, NSURLSession,
    NSURLSessionDataDelegate, NSURLSessionDataTask, NSURLSessionDelegate, NSURLSessionResponseDisposition,
    NSURLSessionTask, NSURLSessionTaskDelegate,
};

use crate::response_head::ResponseHead;
use crate::task_event::TaskEvent;
use crate::transfer_event::TransferEvent;

#[derive(Debug)]
pub(crate) struct DelegateState {
    events: Sender<TaskEvent>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = DelegateState]
    #[derive(Debug)]
    pub(crate) struct SessionDelegate;

    unsafe impl NSObjectProtocol for SessionDelegate {}

    unsafe impl NSURLSessionDelegate for SessionDelegate {}

    unsafe impl NSURLSessionTaskDelegate for SessionDelegate {
        #[unsafe(method(URLSession:task:willPerformHTTPRedirection:newRequest:completionHandler:))]
        fn refuse_redirection(
            &self,
            _session: &NSURLSession,
            _task: &NSURLSessionTask,
            _response: &NSHTTPURLResponse,
            _request: &NSURLRequest,
            completion_handler: &DynBlock<dyn Fn(*mut NSURLRequest)>,
        ) {
            completion_handler.call((std::ptr::null_mut(),));
        }

        #[unsafe(method(URLSession:task:didCompleteWithError:))]
        fn task_finished(&self, _session: &NSURLSession, task: &NSURLSessionTask, error: Option<&NSError>) {
            let failure = error.map(|error| error.localizedDescription().to_string());
            self.deliver(task.taskIdentifier(), TransferEvent::Finished(failure));
        }
    }

    unsafe impl NSURLSessionDataDelegate for SessionDelegate {
        #[unsafe(method(URLSession:dataTask:didReceiveResponse:completionHandler:))]
        fn response_received(
            &self,
            _session: &NSURLSession,
            task: &NSURLSessionDataTask,
            response: &NSURLResponse,
            completion_handler: &DynBlock<dyn Fn(NSURLSessionResponseDisposition)>,
        ) {
            self.deliver(task.taskIdentifier(), TransferEvent::Response(head_of(response)));
            completion_handler.call((NSURLSessionResponseDisposition::Allow,));
        }

        #[unsafe(method(URLSession:dataTask:didReceiveData:))]
        fn data_received(&self, _session: &NSURLSession, task: &NSURLSessionDataTask, data: &NSData) {
            self.deliver(task.taskIdentifier(), TransferEvent::Data(data.to_vec()));
        }
    }
);

impl SessionDelegate {
    pub(crate) fn new(events: Sender<TaskEvent>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateState { events });
        unsafe { msg_send![super(this), init] }
    }

    fn deliver(&self, task: usize, event: TransferEvent) {
        let _ = self.ivars().events.send(TaskEvent { task, event });
    }
}

fn head_of(response: &NSURLResponse) -> ResponseHead {
    let Some(http) = response.downcast_ref::<NSHTTPURLResponse>() else {
        return ResponseHead::default();
    };
    let header = |name: &str| {
        http.valueForHTTPHeaderField(&NSString::from_str(name))
            .map(|value| value.to_string())
    };
    ResponseHead {
        status: u16::try_from(http.statusCode()).unwrap_or(0),
        location: header("Location"),
        etag: header("ETag"),
        content_range: header("Content-Range"),
        repo_commit: header("X-Repo-Commit"),
        linked_etag: header("X-Linked-Etag"),
        linked_size: header("X-Linked-Size"),
    }
}
