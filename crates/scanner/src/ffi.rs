use std::ffi::{CStr, c_char, c_void};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use dispatch2::{DispatchQueue, DispatchRetained};
use objc2_core_foundation::{CFArray, CFString, CFUUID};
use objc2_core_services::{
    FSEventStreamContext, FSEventStreamCreate, FSEventStreamEventFlags, FSEventStreamEventId, FSEventStreamInvalidate,
    FSEventStreamRef, FSEventStreamRelease, FSEventStreamSetDispatchQueue, FSEventStreamStart, FSEventStreamStop,
    FSEventsCopyUUIDForDevice, FSEventsGetCurrentEventId, kFSEventStreamCreateFlagNoDefer,
    kFSEventStreamCreateFlagWatchRoot,
};

const QUEUE_LABEL: &str = "com.domovoi.volume-events";
const LATENCY_SECONDS: f64 = 0.2;

pub(crate) struct RawEvent {
    pub(crate) path: PathBuf,
    pub(crate) flags: u32,
    pub(crate) id: u64,
}

type EventHandler = Box<dyn Fn(RawEvent) + Send + Sync>;

pub(crate) struct RawStream {
    stream: FSEventStreamRef,
    _queue: DispatchRetained<DispatchQueue>,
}

unsafe impl Send for RawStream {}

impl RawStream {
    pub(crate) fn start(root: &Path, since: u64, handler: EventHandler) -> io::Result<Self> {
        let root = root
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "watched path is not UTF-8"))?;
        let root = CFString::from_str(root);
        let paths = CFArray::from_objects(&[&*root]);
        let handler: *mut EventHandler = Box::into_raw(Box::new(handler));
        let mut context = FSEventStreamContext {
            version: 0,
            info: handler.cast(),
            retain: None,
            release: Some(release_handler),
            copyDescription: None,
        };
        let stream = unsafe {
            FSEventStreamCreate(
                None,
                Some(deliver_events),
                &raw mut context,
                paths.as_opaque(),
                since,
                LATENCY_SECONDS,
                kFSEventStreamCreateFlagNoDefer | kFSEventStreamCreateFlagWatchRoot,
            )
        };
        if stream.is_null() {
            unsafe { release_handler(handler.cast_const().cast()) };
            return Err(io::Error::other("FSEventStreamCreate returned NULL"));
        }
        let queue = DispatchQueue::new(QUEUE_LABEL, None);
        unsafe { FSEventStreamSetDispatchQueue(stream, Some(&queue)) };
        if !unsafe { FSEventStreamStart(stream) } {
            unsafe {
                FSEventStreamInvalidate(stream);
                FSEventStreamRelease(stream);
            }
            return Err(io::Error::other("FSEventStreamStart failed"));
        }
        Ok(Self { stream, _queue: queue })
    }
}

impl Drop for RawStream {
    fn drop(&mut self) {
        unsafe {
            FSEventStreamStop(self.stream);
            FSEventStreamInvalidate(self.stream);
            FSEventStreamRelease(self.stream);
        }
    }
}

pub(crate) fn history_store(device: libc::dev_t) -> Option<String> {
    let store = unsafe { FSEventsCopyUUIDForDevice(device) }?;
    CFUUID::new_string(None, Some(&store)).map(|uuid| uuid.to_string())
}

pub(crate) fn current_event_id() -> u64 {
    unsafe { FSEventsGetCurrentEventId() }
}

unsafe extern "C-unwind" fn release_handler(info: *const c_void) {
    drop(unsafe { Box::from_raw(info.cast_mut().cast::<EventHandler>()) });
}

unsafe extern "C-unwind" fn deliver_events(
    _stream: objc2_core_services::ConstFSEventStreamRef,
    info: *mut c_void,
    count: usize,
    paths: NonNull<c_void>,
    flags: NonNull<FSEventStreamEventFlags>,
    ids: NonNull<FSEventStreamEventId>,
) {
    let handler = unsafe { &*info.cast::<EventHandler>() };
    let paths = paths.cast::<*const c_char>().as_ptr();
    for index in 0..count {
        let path = unsafe { CStr::from_ptr(*paths.add(index)) };
        handler(RawEvent {
            path: PathBuf::from(std::ffi::OsStr::from_bytes(path.to_bytes())),
            flags: unsafe { *flags.as_ptr().add(index) },
            id: unsafe { *ids.as_ptr().add(index) },
        });
    }
}
