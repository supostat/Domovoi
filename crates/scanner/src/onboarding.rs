use std::sync::{Condvar, Mutex};
use std::time::Duration;

use crate::cancel_token::CancelToken;

const CANCEL_CHECK_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Default)]
pub struct Onboarding {
    finished: Mutex<bool>,
    changed: Condvar,
}

impl Onboarding {
    pub fn finished() -> Self {
        Self {
            finished: Mutex::new(true),
            changed: Condvar::new(),
        }
    }

    pub fn finish(&self) {
        *self.finished.lock().expect("onboarding state is not poisoned") = true;
        self.changed.notify_all();
    }

    pub fn is_finished(&self) -> bool {
        *self.finished.lock().expect("onboarding state is not poisoned")
    }

    pub(crate) fn wait_until_finished(&self, cancel: &CancelToken) -> bool {
        let mut finished = self.finished.lock().expect("onboarding state is not poisoned");
        while !*finished {
            if cancel.is_cancelled() {
                return false;
            }
            finished = self
                .changed
                .wait_timeout(finished, CANCEL_CHECK_INTERVAL)
                .expect("onboarding state is not poisoned")
                .0;
        }
        true
    }
}
