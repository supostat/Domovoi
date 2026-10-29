use std::cell::RefCell;
use std::io;
use std::ptr::NonNull;
use std::thread::{self, JoinHandle};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes, NSTimer};

use crate::session::Session;

const SAVE_TICK: f64 = 0.05;

#[derive(Debug)]
pub struct QuitJob {
    timer: Retained<NSTimer>,
}

impl QuitJob {
    pub fn start(mut session: Session, on_saved: impl FnOnce(io::Result<()>) + 'static) -> Self {
        let saving = thread::spawn(move || session.persist());
        let pending: RefCell<Option<(JoinHandle<io::Result<()>>, _)>> = RefCell::new(Some((saving, on_saved)));
        let block = RcBlock::new(move |timer: NonNull<NSTimer>| {
            let saved = pending
                .borrow()
                .as_ref()
                .is_some_and(|(saving, _)| saving.is_finished());
            if !saved {
                return;
            }
            unsafe { timer.as_ref() }.invalidate();
            let (saving, on_saved) = pending.borrow_mut().take().expect("the cache is saved once");
            on_saved(saving.join().expect("the saving thread does not panic"));
        });
        let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(SAVE_TICK, true, &block) };
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
        Self { timer }
    }
}

impl Drop for QuitJob {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}
