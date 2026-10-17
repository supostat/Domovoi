use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use domovoi_dups::{DuplicateProgress, SearchError};
use domovoi_scanner::CancelToken;
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::search_stage::SearchStage;
use crate::session::Session;

const PROGRESS_BACKLOG: usize = 64;

#[derive(Debug)]
pub(crate) struct DuplicateJob {
    thread: Option<JoinHandle<(Session, Result<(), SearchError>)>>,
    progress: Receiver<DuplicateProgress>,
    cancel: CancelToken,
    timer: Retained<NSTimer>,
    started: Instant,
    pub stage: SearchStage,
    pub done: u64,
    pub total: u64,
    pub candidates: u64,
}

impl DuplicateJob {
    pub(crate) fn start(mut session: Session, timer: Retained<NSTimer>) -> Self {
        let (sender, progress) = mpsc::sync_channel(PROGRESS_BACKLOG);
        let cancel = CancelToken::default();
        let thread = {
            let cancel = cancel.clone();
            thread::spawn(move || {
                let outcome = session.find_duplicates(Some(sender), cancel);
                (session, outcome)
            })
        };
        Self {
            thread: Some(thread),
            progress,
            cancel,
            timer,
            started: Instant::now(),
            stage: SearchStage::Edges,
            done: 0,
            total: 0,
            candidates: 0,
        }
    }

    pub(crate) fn absorb_progress(&mut self) -> bool {
        let mut changed = false;
        while let Ok(progress) = self.progress.try_recv() {
            self.stage = SearchStage::from(progress.stage);
            self.done = progress.files_done;
            self.total = progress.files_total;
            if self.stage == SearchStage::Edges {
                self.candidates = self.candidates.max(progress.files_total);
            }
            changed = true;
        }
        changed
    }

    pub(crate) fn stop(&self) {
        self.cancel.cancel();
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub(crate) fn seconds(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub(crate) fn finish(mut self) -> (Session, Result<(), SearchError>) {
        self.timer.invalidate();
        self.thread
            .take()
            .expect("a duplicate search finishes once")
            .join()
            .expect("the duplicate search thread does not panic")
    }
}
