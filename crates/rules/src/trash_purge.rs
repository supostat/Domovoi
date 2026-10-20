use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use domovoi_apfs_sys::link_id;
use domovoi_scanner::CancelToken;
use objc2::rc::autoreleasepool;
use objc2_foundation::{NSFileManager, NSString, NSURL};

use crate::purge_progress::PurgeProgress;
use crate::purge_report::PurgeReport;
use crate::purge_skip::PurgeSkip;
use crate::trash_journal::JournalEntry;

pub const PURGE_WORKERS: usize = 4;

struct Planned {
    entry: JournalEntry,
    root: PathBuf,
    children: Vec<PathBuf>,
}

enum Plan {
    Ready(Planned),
    Skipped(JournalEntry, PurgeSkip),
    Failed(JournalEntry, String),
}

impl Planned {
    fn work(&self) -> usize {
        self.children.len() + 1
    }
}

struct Tally<'progress> {
    done: AtomicUsize,
    total: usize,
    entries_done: AtomicUsize,
    progress: &'progress (dyn Fn(PurgeProgress) + Sync),
}

impl Tally<'_> {
    fn step(&self) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        (self.progress)(PurgeProgress {
            done,
            total: self.total,
            entries_done: self.entries_done.load(Ordering::Relaxed),
        });
    }

    fn finish_entry(&self) {
        let entries_done = self.entries_done.fetch_add(1, Ordering::Relaxed) + 1;
        (self.progress)(PurgeProgress {
            done: self.done.load(Ordering::Relaxed),
            total: self.total,
            entries_done,
        });
    }
}

pub fn purge(
    entries: Vec<JournalEntry>,
    cancel: &CancelToken,
    progress: &(dyn Fn(PurgeProgress) + Sync),
) -> PurgeReport {
    let mut report = PurgeReport::default();
    let mut planned = Vec::new();
    for entry in entries {
        match plan(entry) {
            Plan::Ready(item) => planned.push(item),
            Plan::Skipped(entry, skip) => report.skipped.push((entry, skip)),
            Plan::Failed(entry, error) => report.failed.push((entry, error)),
        }
    }
    let tally = Tally {
        done: AtomicUsize::new(0),
        total: planned.iter().map(Planned::work).sum(),
        entries_done: AtomicUsize::new(0),
        progress,
    };
    let mut remaining = planned.into_iter();
    for item in remaining.by_ref() {
        if cancel.is_cancelled() {
            report.untouched.push(item.entry);
            break;
        }
        match remove_entry(&item, cancel, &tally) {
            Ok(true) => report.purged.push(item.entry),
            Ok(false) => report.interrupted.push(item.entry),
            Err(error) => report.failed.push((item.entry, error)),
        }
        tally.finish_entry();
    }
    report.untouched.extend(remaining.map(|item| item.entry));
    report
}

fn plan(entry: JournalEntry) -> Plan {
    let root = PathBuf::from(&entry.trashed);
    let metadata = match root.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Plan::Skipped(entry, PurgeSkip::NotInTrash),
        Err(error) => return Plan::Failed(entry, error.to_string()),
    };
    match link_id(&root) {
        Ok(current) if current == entry.link_id => {}
        Ok(_) => return Plan::Skipped(entry, PurgeSkip::Replaced),
        Err(error) => return Plan::Failed(entry, error.to_string()),
    }
    if !metadata.is_dir() {
        return Plan::Ready(Planned {
            entry,
            root,
            children: Vec::new(),
        });
    }
    match fs::read_dir(&root).and_then(|listing| listing.map(|child| child.map(|child| child.path())).collect()) {
        Ok(children) => Plan::Ready(Planned { entry, root, children }),
        Err(error) => Plan::Failed(entry, error.to_string()),
    }
}

fn remove_entry(item: &Planned, cancel: &CancelToken, tally: &Tally<'_>) -> Result<bool, String> {
    let next = AtomicUsize::new(0);
    let first_failure = Mutex::new(None);
    thread::scope(|scope| {
        for _ in 0..PURGE_WORKERS.min(item.children.len()) {
            scope.spawn(|| {
                while !cancel.is_cancelled() {
                    let Some(child) = item.children.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    if let Err(error) = remove(child) {
                        first_failure
                            .lock()
                            .expect("the failure slot is not poisoned")
                            .get_or_insert(error);
                    }
                    tally.step();
                }
            });
        }
    });
    if let Some(error) = first_failure.into_inner().expect("the failure slot is not poisoned") {
        return Err(error);
    }
    if next.load(Ordering::Relaxed) < item.children.len() {
        return Ok(false);
    }
    remove(&item.root)?;
    tally.step();
    Ok(true)
}

fn remove(path: &Path) -> Result<(), String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("{} is not UTF-8", path.display()))?;
    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(text));
        NSFileManager::defaultManager()
            .removeItemAtURL_error(&url)
            .map_err(|error| error.localizedDescription().to_string())
    })
}
