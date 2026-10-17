use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

use domovoi_scanner::{CancelToken, ScanProgress};
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::scan_status::ScanStatus;
use crate::session::Session;
use crate::session_error::SessionError;

const PROGRESS_BACKLOG: usize = 64;

#[derive(Debug)]
pub(crate) struct ScanJob {
    thread: Option<JoinHandle<(Session, Result<(), SessionError>)>>,
    progress: Receiver<ScanProgress>,
    cancel: CancelToken,
    timer: Retained<NSTimer>,
    pub status: ScanStatus,
    pub expected_bytes: u64,
}

impl ScanJob {
    pub(crate) fn start(
        mut session: Session,
        full_first_scan: bool,
        expected_bytes: u64,
        timer: Retained<NSTimer>,
    ) -> Self {
        let (sender, progress) = mpsc::sync_channel(PROGRESS_BACKLOG);
        let cancel = CancelToken::default();
        let status = ScanStatus {
            bytes: 0,
            files: 0,
            sweep: 0.0,
            threads: session.workers(),
            current: session.root().display().to_string(),
            full_first_scan,
        };
        let thread = {
            let cancel = cancel.clone();
            thread::spawn(move || {
                let outcome = session.scan(Some(sender), cancel);
                (session, outcome)
            })
        };
        Self {
            thread: Some(thread),
            progress,
            cancel,
            timer,
            status,
            expected_bytes,
        }
    }

    pub(crate) fn absorb_progress(&mut self) -> bool {
        let mut changed = false;
        while let Ok(progress) = self.progress.try_recv() {
            self.status.bytes = progress.bytes;
            self.status.files = progress.files;
            self.status.current = progress.current.display().to_string();
            changed = true;
        }
        if changed {
            self.status.sweep = (self.status.bytes as f64 / self.expected_bytes.max(1) as f64).min(1.0) as f32;
        }
        changed
    }

    pub(crate) fn stop(&self) {
        self.cancel.cancel();
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub(crate) fn finish(mut self) -> (Session, Result<(), SessionError>) {
        self.timer.invalidate();
        self.thread
            .take()
            .expect("a scan job finishes once")
            .join()
            .expect("the scan thread does not panic")
    }
}
