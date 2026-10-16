use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SendError, Sender};

use objc2_core_services::{
    kFSEventStreamEventFlagHistoryDone, kFSEventStreamEventFlagKernelDropped, kFSEventStreamEventFlagMustScanSubDirs,
    kFSEventStreamEventFlagRootChanged, kFSEventStreamEventFlagUnmount, kFSEventStreamEventFlagUserDropped,
    kFSEventStreamEventIdSinceNow,
};

use crate::ffi::{self, RawEvent, RawStream};

const SUBTREE_RESCAN_FLAGS: u32 =
    kFSEventStreamEventFlagMustScanSubDirs | kFSEventStreamEventFlagUserDropped | kFSEventStreamEventFlagKernelDropped;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Since {
    Now,
    Event(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VolumeEvent {
    Changed {
        path: PathBuf,
        rescan_subtree: bool,
        id: u64,
    },
    HistoryDone {
        id: u64,
    },
    RootChanged,
    Unmount {
        path: PathBuf,
    },
}

pub struct VolumeEventStream {
    _stream: RawStream,
}

impl VolumeEventStream {
    pub fn watch(root: &Path, since: Since, events: Sender<VolumeEvent>) -> io::Result<Self> {
        let since = match since {
            Since::Now => kFSEventStreamEventIdSinceNow,
            Since::Event(id) => id,
        };
        let stream = RawStream::start(
            root,
            since,
            Box::new(move |raw| match events.send(VolumeEvent::from_raw(raw)) {
                Ok(()) | Err(SendError(_)) => {}
            }),
        )?;
        Ok(Self { _stream: stream })
    }
}

impl fmt::Debug for VolumeEventStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("VolumeEventStream").finish_non_exhaustive()
    }
}

impl VolumeEvent {
    fn from_raw(raw: RawEvent) -> Self {
        let RawEvent { path, flags, id } = raw;
        if flags & kFSEventStreamEventFlagUnmount != 0 {
            Self::Unmount { path }
        } else if flags & kFSEventStreamEventFlagHistoryDone != 0 {
            Self::HistoryDone { id }
        } else if flags & kFSEventStreamEventFlagRootChanged != 0 {
            Self::RootChanged
        } else {
            Self::Changed {
                path,
                rescan_subtree: flags & SUBTREE_RESCAN_FLAGS != 0,
                id,
            }
        }
    }
}

pub fn history_store(volume_root: &Path) -> io::Result<Option<String>> {
    let device = libc::dev_t::try_from(fs::metadata(volume_root)?.dev())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "device number does not fit dev_t"))?;
    Ok(ffi::history_store(device))
}

pub fn current_event_id() -> u64 {
    ffi::current_event_id()
}
