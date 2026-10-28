use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use domovoi_apfs_sys::sys::{DIR_MNTSTATUS_MNTPOINT, DIR_MNTSTATUS_TRIGGER, SF_FIRMLINK};
use domovoi_apfs_sys::{
    AccessError, AttributeSet, BulkListing, EntryAttributes, ObjectKind, link_id, open_at, unlink_at,
};
use domovoi_scanner::CancelToken;

use crate::delete_failure::DeleteFailure;
use crate::delete_progress::DeleteProgress;
use crate::delete_report::DeleteReport;
use crate::delete_subject::DeleteSubject;

pub const DELETE_WORKERS: usize = 4;

struct Planned {
    subject: DeleteSubject,
    is_directory: bool,
}

enum Plan {
    Ready(Planned),
    Failed(DeleteSubject, DeleteFailure),
}

enum Removal {
    Removed,
    Stopped,
}

struct Child {
    name: OsString,
    kind: ChildKind,
}

enum ChildKind {
    Leaf,
    Directory,
    OtherVolume,
    Unreadable(DeleteFailure),
}

impl ChildKind {
    fn of(attributes: &EntryAttributes) -> Self {
        let mount_point = attributes
            .mount_status
            .is_some_and(|status| status & (DIR_MNTSTATUS_MNTPOINT | DIR_MNTSTATUS_TRIGGER) != 0);
        if attributes.kind != ObjectKind::Directory {
            Self::Leaf
        } else if mount_point || attributes.flags & SF_FIRMLINK != 0 {
            Self::OtherVolume
        } else {
            Self::Directory
        }
    }
}

struct Tally<'progress> {
    files_done: AtomicUsize,
    files_total: usize,
    subjects_done: AtomicUsize,
    progress: &'progress (dyn Fn(DeleteProgress) + Sync),
}

impl Tally<'_> {
    fn step(&self) {
        let files_done = self.files_done.fetch_add(1, Ordering::Relaxed) + 1;
        self.report(files_done, self.subjects_done.load(Ordering::Relaxed));
    }

    fn finish_subject(&self) {
        let subjects_done = self.subjects_done.fetch_add(1, Ordering::Relaxed) + 1;
        self.report(self.files_done.load(Ordering::Relaxed), subjects_done);
    }

    fn report(&self, files_done: usize, subjects_done: usize) {
        (self.progress)(DeleteProgress {
            files_done,
            files_total: self.files_total.max(files_done),
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
    let files_total = planned.iter().map(|item| item.subject.indexed_files).sum::<u64>();
    let tally = Tally {
        files_done: AtomicUsize::new(0),
        files_total: usize::try_from(files_total).expect("a file count fits in usize"),
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
            Ok(Removal::Removed) => {
                report.freed_bytes += item.subject.measured_bytes;
                report.deleted.push(item.subject);
            }
            Ok(Removal::Stopped) => report.interrupted.push(item.subject),
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
    Plan::Ready(Planned {
        is_directory: metadata.is_dir(),
        subject,
    })
}

fn remove_subject(item: &Planned, cancel: &CancelToken, tally: &Tally<'_>) -> Result<Removal, DeleteFailure> {
    let physical = &item.subject.physical;
    let name = physical.file_name().expect("a deleted item has a name");
    let parent = File::open(physical.parent().expect("a deleted item has a parent"))
        .map_err(|error| DeleteFailure::from_io(&error))?;
    if !item.is_directory {
        unlink(parent.as_fd(), name, 0)?;
        tally.step();
        return Ok(Removal::Removed);
    }
    let directory = open_directory(parent.as_fd(), name).map_err(|error| DeleteFailure::from_io(&error))?;
    let children = children(directory.as_fd())?;
    let next = AtomicUsize::new(0);
    let stopped = AtomicBool::new(false);
    let first_failure = Mutex::new(None);
    thread::scope(|scope| {
        for _ in 0..DELETE_WORKERS.min(children.len()) {
            scope.spawn(|| {
                while !cancel.is_cancelled() {
                    let Some(child) = children.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    match remove_child(directory.as_fd(), child, cancel, tally) {
                        Ok(Removal::Removed) => {}
                        Ok(Removal::Stopped) => stopped.store(true, Ordering::Relaxed),
                        Err(failure) => {
                            first_failure
                                .lock()
                                .expect("the failure slot is not poisoned")
                                .get_or_insert(failure);
                        }
                    }
                }
            });
        }
    });
    if let Some(failure) = first_failure.into_inner().expect("the failure slot is not poisoned") {
        return Err(failure);
    }
    if stopped.load(Ordering::Relaxed) || next.load(Ordering::Relaxed) < children.len() {
        return Ok(Removal::Stopped);
    }
    drop(directory);
    unlink(parent.as_fd(), name, libc::AT_REMOVEDIR)?;
    Ok(Removal::Removed)
}

fn remove_child(
    directory: BorrowedFd<'_>,
    child: &Child,
    cancel: &CancelToken,
    tally: &Tally<'_>,
) -> Result<Removal, DeleteFailure> {
    match &child.kind {
        ChildKind::Leaf => {
            unlink(directory, &child.name, 0)?;
            tally.step();
            Ok(Removal::Removed)
        }
        ChildKind::Directory => remove_directory(directory, &child.name, cancel, tally),
        ChildKind::OtherVolume => Err(DeleteFailure::from_io(&io::Error::from_raw_os_error(libc::EXDEV))),
        ChildKind::Unreadable(failure) => Err(failure.clone()),
    }
}

fn remove_directory(
    parent: BorrowedFd<'_>,
    name: &OsStr,
    cancel: &CancelToken,
    tally: &Tally<'_>,
) -> Result<Removal, DeleteFailure> {
    let directory = match open_directory(parent, name) {
        Ok(directory) => directory,
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(Removal::Removed),
        Err(error) => return Err(DeleteFailure::from_io(&error)),
    };
    let mut first_failure = None;
    for child in &children(directory.as_fd())? {
        if cancel.is_cancelled() {
            return first_failure.map_or(Ok(Removal::Stopped), Err);
        }
        match remove_child(directory.as_fd(), child, cancel, tally) {
            Ok(Removal::Removed) => {}
            Ok(Removal::Stopped) => return first_failure.map_or(Ok(Removal::Stopped), Err),
            Err(failure) => {
                first_failure.get_or_insert(failure);
            }
        }
    }
    if let Some(failure) = first_failure {
        return Err(failure);
    }
    drop(directory);
    unlink(parent, name, libc::AT_REMOVEDIR)?;
    Ok(Removal::Removed)
}

fn children(directory: BorrowedFd<'_>) -> Result<Vec<Child>, DeleteFailure> {
    let mut children = Vec::new();
    for entry in BulkListing::new(directory, AttributeSet::Base) {
        let entry = entry.map_err(|error| DeleteFailure::from_io(&error))?;
        let kind = match &entry.attributes {
            Ok(attributes) => ChildKind::of(attributes),
            Err(AccessError::Missing) => continue,
            Err(error) => ChildKind::Unreadable(DeleteFailure::from_access(error)),
        };
        children.push(Child { name: entry.name, kind });
    }
    Ok(children)
}

fn open_directory(parent: BorrowedFd<'_>, name: &OsStr) -> io::Result<OwnedFd> {
    open_at(parent, name, libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW, 0)
}

fn unlink(directory: BorrowedFd<'_>, name: &OsStr, flags: libc::c_int) -> Result<(), DeleteFailure> {
    match unlink_at(directory, name, flags) {
        Err(error) if error.raw_os_error() != Some(libc::ENOENT) => Err(DeleteFailure::from_io(&error)),
        _ => Ok(()),
    }
}
