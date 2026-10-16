use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process;
use std::sync::Arc;

use dispatch2::{
    _dispatch_source_type_memorypressure, DispatchObject, DispatchQueue, DispatchRetained, DispatchSource,
};

use crate::inference_service::InferenceService;
use crate::pressure_level::PressureLevel;

const NORMAL: usize = 0x1;
const WARNING: usize = 0x2;
const CRITICAL: usize = 0x4;
const QUEUE_LABEL: &str = "com.domovoi.inference.memory-pressure";

struct PressureContext {
    service: Arc<InferenceService>,
    source: *const DispatchSource,
}

#[derive(Debug)]
pub struct MemoryPressureMonitor {
    source: DispatchRetained<DispatchSource>,
    context: *mut c_void,
}

unsafe impl Send for MemoryPressureMonitor {}

impl MemoryPressureMonitor {
    pub fn start(service: Arc<InferenceService>) -> Self {
        let queue = DispatchQueue::new(QUEUE_LABEL, None);
        let source = unsafe {
            DispatchSource::new(
                (&raw const _dispatch_source_type_memorypressure).cast_mut(),
                0,
                NORMAL | WARNING | CRITICAL,
                Some(&queue),
            )
        };
        let context = Box::into_raw(Box::new(PressureContext {
            service,
            source: &raw const *source,
        }))
        .cast::<c_void>();
        unsafe { source.set_context(context) };
        source.set_event_handler_f(report_pressure);
        source.activate();
        Self { source, context }
    }
}

impl Drop for MemoryPressureMonitor {
    fn drop(&mut self) {
        self.source.cancel();
        drop(unsafe { Box::from_raw(self.context.cast::<PressureContext>()) });
    }
}

extern "C" fn report_pressure(context: *mut c_void) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let context = unsafe { &*context.cast::<PressureContext>() };
        let flags = unsafe { &*context.source }.data();
        let level = if flags & CRITICAL != 0 {
            PressureLevel::Critical
        } else if flags & WARNING != 0 {
            PressureLevel::Warning
        } else {
            PressureLevel::Normal
        };
        context.service.report_pressure(level);
    }));
    if outcome.is_err() {
        process::abort();
    }
}
