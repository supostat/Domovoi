use std::thread::{self, JoinHandle};

use crate::applied_batch::AppliedBatch;
use crate::read_batch::ReadBatch;
use crate::session::Session;

#[derive(Debug)]
pub struct ApplyJob {
    thread: Option<JoinHandle<(Session, AppliedBatch)>>,
}

impl ApplyJob {
    pub fn start(mut session: Session, batch: ReadBatch) -> Self {
        let thread = thread::spawn(move || {
            let applied = batch.apply(&mut session);
            (session, applied)
        });
        Self { thread: Some(thread) }
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub fn finish(mut self) -> (Session, AppliedBatch) {
        self.thread
            .take()
            .expect("a batch is applied once")
            .join()
            .expect("the applying thread does not panic")
    }
}
