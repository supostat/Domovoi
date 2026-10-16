use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Default)]
pub struct CancelFlag {
    raised: Arc<AtomicBool>,
}

impl CancelFlag {
    pub fn raise(&self) {
        self.raised.store(true, Ordering::SeqCst);
    }

    pub fn lower(&self) {
        self.raised.store(false, Ordering::SeqCst);
    }

    pub fn is_raised(&self) -> bool {
        self.raised.load(Ordering::SeqCst)
    }

    pub(crate) fn as_callback_data(&self) -> *mut c_void {
        Arc::as_ptr(&self.raised).cast_mut().cast()
    }

    pub(crate) unsafe fn is_raised_at(data: *mut c_void) -> bool {
        unsafe { &*data.cast::<AtomicBool>() }.load(Ordering::SeqCst)
    }
}
