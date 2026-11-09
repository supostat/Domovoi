use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

use domovoi_index::NodeId;
use domovoi_scanner::CancelToken;

use crate::clone_progress::CloneProgress;
use crate::clones_outcome::ClonesOutcome;
use crate::session::Session;
use crate::session_error::SessionError;

const PROGRESS_BACKLOG: usize = 64;

#[derive(Debug)]
pub struct CloneJob {
    thread: Option<JoinHandle<(Session, Result<ClonesOutcome, SessionError>)>>,
    progress: Receiver<CloneProgress>,
    cancel: CancelToken,
    pub done: u64,
    pub total: u64,
}

impl CloneJob {
    pub fn start(mut session: Session, pairs: Vec<(NodeId, NodeId)>) -> Self {
        let total = session.clone_bytes(&pairs);
        let (sender, progress) = mpsc::sync_channel(PROGRESS_BACKLOG);
        let cancel = CancelToken::default();
        let thread = {
            let cancel = cancel.clone();
            thread::spawn(move || {
                let outcome = session.replace_with_clones(&pairs, Some(&sender), &cancel);
                (session, outcome)
            })
        };
        Self {
            thread: Some(thread),
            progress,
            cancel,
            done: 0,
            total,
        }
    }

    pub fn absorb_progress(&mut self) -> bool {
        let mut changed = false;
        while let Ok(progress) = self.progress.try_recv() {
            self.done = progress.done;
            self.total = progress.total;
            changed = true;
        }
        changed
    }

    pub fn stop(&self) {
        self.cancel.cancel();
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub fn finish(mut self) -> (Session, Result<ClonesOutcome, SessionError>) {
        self.thread
            .take()
            .expect("copies are replaced once")
            .join()
            .expect("the cloning thread does not panic")
    }
}
