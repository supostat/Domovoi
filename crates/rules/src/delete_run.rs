use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use domovoi_apfs_sys::link_id;
use domovoi_scanner::CancelToken;
use objc2::rc::autoreleasepool;
use objc2_foundation::{NSFileManager, NSString, NSURL};

use crate::delete_failure::DeleteFailure;
use crate::delete_progress::DeleteProgress;
use crate::delete_report::DeleteReport;
use crate::delete_subject::DeleteSubject;

pub const DELETE_WORKERS: usize = 4;

struct Planned {
    subject: DeleteSubject,
    children: Vec<PathBuf>,
}

enum Plan {
    Ready(Planned),
    Failed(DeleteSubject, DeleteFailure),
}

impl Planned {
    fn work(&self) -> usize {
        self.children.len() + 1
    }
}

struct Tally<'progress> {
    done: AtomicUsize,
    total: usize,
    subjects_done: AtomicUsize,
    progress: &'progress (dyn Fn(DeleteProgress) + Sync),
}

impl Tally<'_> {
    fn step(&self) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        (self.progress)(DeleteProgress {
            done,
            total: self.total,
            subjects_done: self.subjects_done.load(Ordering::Relaxed),
        });
    }

    fn finish_subject(&self) {
        let subjects_done = self.subjects_done.fetch_add(1, Ordering::Relaxed) + 1;
        (self.progress)(DeleteProgress {
            done: self.done.load(Ordering::Relaxed),
            total: self.total,
            subjects_done,
        });
    }
}

pub fn delete(
    subjects: Vec<DeleteSubject>,
    cancel: &CancelToken,
    progress: &(dyn Fn(DeleteProgress) + Sync),
) -> DeleteReport {
    let mut report = DeleteReport::default();
    let mut planned = Vec::new();
    for subject in subjects {
        match plan(subject) {
            Plan::Ready(item) => planned.push(item),
            Plan::Failed(subject, failure) => report.failed.push((subject, failure)),
        }
    }
    let tally = Tally {
        done: AtomicUsize::new(0),
        total: planned.iter().map(Planned::work).sum(),
        subjects_done: AtomicUsize::new(0),
        progress,
    };
    let mut remaining = planned.into_iter();
    for item in remaining.by_ref() {
        if cancel.is_cancelled() {
            report.untouched.push(item.subject);
            break;
        }
        match remove_subject(&item, cancel, &tally) {
            Ok(true) => {
                report.freed_bytes += item.subject.measured_bytes;
                report.deleted.push(item.subject);
            }
            Ok(false) => report.interrupted.push(item.subject),
            Err(failure) => report.failed.push((item.subject, failure)),
        }
        tally.finish_subject();
    }
    report.untouched.extend(remaining.map(|item| item.subject));
    report
}

fn plan(subject: DeleteSubject) -> Plan {
    let metadata = match subject.physical.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) => return Plan::Failed(subject, DeleteFailure::from_io(&error)),
    };
    match link_id(&subject.physical) {
        Ok(current) if current == subject.link_id => {}
        Ok(_) => return Plan::Failed(subject, DeleteFailure::Replaced),
        Err(error) => return Plan::Failed(subject, DeleteFailure::from_io(&error)),
    }
    if !metadata.is_dir() {
        return Plan::Ready(Planned {
            subject,
            children: Vec::new(),
        });
    }
    let listing = fs::read_dir(&subject.physical)
        .and_then(|listing| listing.map(|child| child.map(|child| child.path())).collect());
    match listing {
        Ok(children) => Plan::Ready(Planned { subject, children }),
        Err(error) => Plan::Failed(subject, DeleteFailure::from_io(&error)),
    }
}

fn remove_subject(item: &Planned, cancel: &CancelToken, tally: &Tally<'_>) -> Result<bool, DeleteFailure> {
    let next = AtomicUsize::new(0);
    let first_failure = Mutex::new(None);
    thread::scope(|scope| {
        for _ in 0..DELETE_WORKERS.min(item.children.len()) {
            scope.spawn(|| {
                while !cancel.is_cancelled() {
                    let Some(child) = item.children.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    if let Err(failure) = remove(child) {
                        first_failure
                            .lock()
                            .expect("the failure slot is not poisoned")
                            .get_or_insert(failure);
                    }
                    tally.step();
                }
            });
        }
    });
    if let Some(failure) = first_failure.into_inner().expect("the failure slot is not poisoned") {
        return Err(failure);
    }
    if next.load(Ordering::Relaxed) < item.children.len() {
        return Ok(false);
    }
    remove(&item.subject.physical)?;
    tally.step();
    Ok(true)
}

fn remove(path: &Path) -> Result<(), DeleteFailure> {
    let text = path.to_str().ok_or_else(|| DeleteFailure::Other {
        code: 0,
        description: format!("{} is not UTF-8", path.display()),
    })?;
    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(text));
        NSFileManager::defaultManager()
            .removeItemAtURL_error(&url)
            .map_err(|error| DeleteFailure::from_cocoa(error.code(), error.localizedDescription().to_string()))
    })
}
