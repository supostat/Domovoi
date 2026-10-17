use std::sync::mpsc::Receiver;

use domovoi_scanner::{VolumeEvent, VolumeEventStream};
use objc2::rc::Retained;
use objc2_foundation::NSTimer;

#[derive(Debug)]
pub(crate) struct EventWatch {
    pub _stream: VolumeEventStream,
    pub events: Receiver<VolumeEvent>,
    pub timer: Retained<NSTimer>,
}

impl Drop for EventWatch {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}
