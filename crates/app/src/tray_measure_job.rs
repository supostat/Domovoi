use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use domovoi_apfs_sys::{AttributeSet, path_entry};
use domovoi_index::MeasuredSet;
use domovoi_scanner::{CancelToken, Node, NodeState, ScanError, ScanOptions, ScanProgress, scan};

use crate::measure_progress::MeasureProgress;
use crate::tray_measured::TrayMeasured;
use crate::tray_sizes::TraySizes;

const PROGRESS_BACKLOG: usize = 64;

#[derive(Debug)]
pub struct TrayMeasureJob {
    thread: Option<JoinHandle<Result<TrayMeasured, ScanError>>>,
    progress: Receiver<MeasureProgress>,
    cancel: CancelToken,
    latest: MeasureProgress,
}

impl TrayMeasureJob {
    pub fn start(items: Vec<PathBuf>) -> Self {
        let (sender, progress) = mpsc::sync_channel(PROGRESS_BACKLOG);
        let cancel = CancelToken::default();
        let thread = {
            let cancel = cancel.clone();
            thread::spawn(move || measure_items(&items, &sender, &cancel))
        };
        Self {
            thread: Some(thread),
            progress,
            cancel,
            latest: MeasureProgress::default(),
        }
    }

    pub fn progress(&mut self) -> MeasureProgress {
        if let Some(latest) = self.progress.try_iter().last() {
            self.latest = latest;
        }
        self.latest
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn finish(mut self) -> Result<TrayMeasured, ScanError> {
        self.thread
            .take()
            .expect("a measurement finishes once")
            .join()
            .expect("the measuring thread does not panic")
    }
}

fn measure_items(
    items: &[PathBuf],
    progress: &SyncSender<MeasureProgress>,
    cancel: &CancelToken,
) -> Result<TrayMeasured, ScanError> {
    let mut walked = MeasureProgress::default();
    let mut sizes = Vec::with_capacity(items.len());
    let mut together = MeasuredSet::default();
    for item in items {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let (measured, after) = measure_item(item, walked, progress, cancel)?;
        walked = after;
        sizes.push(measured.reclaimable());
        together.absorb(measured);
    }
    Ok(TrayMeasured {
        sizes: TraySizes {
            items: sizes,
            together: together.reclaimable(),
        },
        walked,
    })
}

fn measure_item(
    item: &Path,
    before: MeasureProgress,
    progress: &SyncSender<MeasureProgress>,
    cancel: &CancelToken,
) -> Result<(MeasuredSet, MeasureProgress), ScanError> {
    match fs::symlink_metadata(item) {
        Ok(metadata) if metadata.is_dir() => measure_folder(item, before, progress, cancel),
        Ok(_) => measure_file(item, before, progress),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((MeasuredSet::default(), before)),
        Err(error) => Err(ScanError::Root(error)),
    }
}

fn measure_file(
    item: &Path,
    before: MeasureProgress,
    progress: &SyncSender<MeasureProgress>,
) -> Result<(MeasuredSet, MeasureProgress), ScanError> {
    let entry = match path_entry(item, AttributeSet::ExtendedWithPrivateSize) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((MeasuredSet::default(), before)),
        Err(error) => return Err(ScanError::Root(error)),
    };
    let attributes = entry.attributes.ok();
    let walked = MeasureProgress {
        files: before.files + 1,
        bytes: before.bytes
            + attributes
                .as_ref()
                .and_then(|attributes| attributes.alloc_size)
                .unwrap_or(0),
    };
    report(progress, walked);
    let node = Node {
        name: entry.name,
        attributes,
        last_used: None,
        state: NodeState::Leaf,
        children: Vec::new(),
    };
    Ok((MeasuredSet::of(&node), walked))
}

fn measure_folder(
    item: &Path,
    before: MeasureProgress,
    progress: &SyncSender<MeasureProgress>,
    cancel: &CancelToken,
) -> Result<(MeasuredSet, MeasureProgress), ScanError> {
    thread::scope(|scope| {
        let (sender, events) = mpsc::sync_channel(PROGRESS_BACKLOG);
        let options = ScanOptions {
            attributes: AttributeSet::ExtendedWithPrivateSize,
            progress: Some(sender),
            cancel: cancel.clone(),
            ..ScanOptions::default()
        };
        let walk = scope.spawn(move || scan(item, &options));
        for event in events {
            let (ScanProgress::Walking { files, bytes, .. } | ScanProgress::Walked { files, bytes }) = event;
            report(
                progress,
                MeasureProgress {
                    files: before.files + files,
                    bytes: before.bytes + bytes,
                },
            );
        }
        match walk.join().expect("the measuring walk does not panic") {
            Ok(walked) => Ok((
                MeasuredSet::of(&walked.root),
                MeasureProgress {
                    files: before.files + walked.files,
                    bytes: before.bytes + walked.bytes,
                },
            )),
            Err(ScanError::Root(_)) => Ok((MeasuredSet::default(), before)),
            Err(error) => Err(error),
        }
    })
}

fn report(progress: &SyncSender<MeasureProgress>, measured: MeasureProgress) {
    match progress.try_send(measured) {
        Ok(()) | Err(mpsc::TrySendError::Full(_) | mpsc::TrySendError::Disconnected(_)) => {}
    }
}
