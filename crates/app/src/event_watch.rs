use std::sync::mpsc::Receiver;

use domovoi_scanner::{VolumeEvent, VolumeEventStream};
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

use crate::event_queue::EventQueue;

#[derive(Debug)]
pub(crate) struct EventWatch {
    pub _stream: VolumeEventStream,
    pub events: Receiver<VolumeEvent>,
    pub timer: Retained<NSTimer>,
    pub queue: EventQueue,
}

impl Drop for EventWatch {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}
