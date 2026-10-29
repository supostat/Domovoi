use std::ffi::{CStr, c_void};
use std::io;
use std::path::Path;
use std::ptr::{self, NonNull};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2_core_foundation::{CFError, CFRetained, CFType, CFURL};

const ESTIMATE_SYMBOL: &CStr = c"CSDiskSpaceGetRecoveryEstimate";
const START_SYMBOL: &CStr = c"CSDiskSpaceStartRecovery";
const CANCEL_SYMBOL: &CStr = c"CSDiskSpaceCancelRecovery";
const QUEUE_LABEL: &str = "com.domovoi.index.space-recovery";
const OPTIONS_PASSED_BY_KNOWN_CALLERS: u32 = 1;

type EstimateFn = unsafe extern "C" fn(volume: *const CFURL) -> i64;
type StartFn = unsafe extern "C" fn(
    volume: *const CFURL,
    bytes: i64,
    options: u32,
    operation: *mut *const CFType,
    queue: *const DispatchQueue,
    completion: *const c_void,
);
type CancelFn = unsafe extern "C" fn(operation: *const CFType);
type Completion = RcBlock<dyn Fn(u8, u64, *mut c_void)>;

#[derive(Clone, Copy, Debug)]
pub struct SpaceRecovery {
    estimate: EstimateFn,
    start: StartFn,
    cancel: CancelFn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryOutcome {
    pub succeeded: bool,
    pub available: u64,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct RecoveryHandle {
    operation: Option<CFRetained<CFType>>,
    finished: Option<RecoveryOutcome>,
    outcomes: Receiver<RecoveryOutcome>,
    _completion: Completion,
    _queue: DispatchRetained<DispatchQueue>,
}

impl SpaceRecovery {
    pub fn load() -> Option<Self> {
        let estimate = symbol(ESTIMATE_SYMBOL)?;
        let start = symbol(START_SYMBOL)?;
        let cancel = symbol(CANCEL_SYMBOL)?;
        Some(unsafe {
            Self {
                estimate: std::mem::transmute::<NonNull<c_void>, EstimateFn>(estimate),
                start: std::mem::transmute::<NonNull<c_void>, StartFn>(start),
                cancel: std::mem::transmute::<NonNull<c_void>, CancelFn>(cancel),
            }
        })
    }

    pub fn estimate(self, volume: &Path) -> io::Result<u64> {
        let url = volume_url(volume)?;
        let bytes = unsafe { (self.estimate)(ptr::from_ref(&*url)) };
        u64::try_from(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "the recovery estimate is negative"))
    }

    pub fn start(self, volume: &Path, bytes: u64) -> io::Result<RecoveryHandle> {
        let url = volume_url(volume)?;
        let bytes = i64::try_from(bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the requested size exceeds what the mechanism takes",
            )
        })?;
        let queue = DispatchQueue::new(QUEUE_LABEL, None);
        let (report, outcomes) = mpsc::channel();
        let completion: Completion = RcBlock::new(move |succeeded: u8, available: u64, error: *mut c_void| {
            let outcome = RecoveryOutcome {
                succeeded: succeeded != 0,
                available,
                error: error_text(error),
            };
            let _ = report.send(outcome);
        });
        let mut operation: *const CFType = ptr::null();
        unsafe {
            (self.start)(
                ptr::from_ref(&*url),
                bytes,
                OPTIONS_PASSED_BY_KNOWN_CALLERS,
                &raw mut operation,
                ptr::from_ref(&*queue),
                ptr::from_ref(&*completion).cast::<c_void>(),
            );
        }
        let operation = NonNull::new(operation.cast_mut()).map(|operation| unsafe { CFRetained::retain(operation) });
        Ok(RecoveryHandle {
            operation,
            finished: None,
            outcomes,
            _completion: completion,
            _queue: queue,
        })
    }

    pub fn cancel(self, handle: &RecoveryHandle) {
        if let Some(operation) = &handle.operation {
            unsafe { (self.cancel)(ptr::from_ref(&**operation)) };
        }
    }
}

impl RecoveryHandle {
    pub fn is_running(&self) -> bool {
        self.operation.is_some() && self.finished.is_none()
    }

    pub fn poll(&mut self) -> Option<&RecoveryOutcome> {
        if self.finished.is_none() {
            self.finished = self.outcomes.try_recv().ok();
        }
        self.finished.as_ref()
    }

    pub fn wait(&mut self, timeout: Duration) -> Option<&RecoveryOutcome> {
        if self.finished.is_none() {
            self.finished = self.outcomes.recv_timeout(timeout).ok();
        }
        self.finished.as_ref()
    }
}

fn symbol(name: &CStr) -> Option<NonNull<c_void>> {
    NonNull::new(unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) })
}

fn volume_url(volume: &Path) -> io::Result<CFRetained<CFURL>> {
    CFURL::from_directory_path(volume)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "volume path does not form a URL"))
}

fn error_text(error: *mut c_void) -> Option<String> {
    let error = NonNull::new(error.cast::<CFError>())?;
    let error = unsafe { error.as_ref() };
    Some(error.description().map_or_else(
        || format!("CoreServices refused the recovery with code {}", error.code()),
        |description| description.to_string(),
    ))
}
