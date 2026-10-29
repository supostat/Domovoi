use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use domovoi_app::{CacheOpening, QuitJob, Session, SessionPaths};
use domovoi_index::Index;
use domovoi_scanner::CancelToken;
use domovoi_testkit::TempTree;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSModalPanelRunLoopMode};
use objc2_foundation::{NSDate, NSRunLoop};

const APP_VERSION: &str = "0.0.0-test";
const NOTE_FILES: usize = 40;
const SAVE_LIMIT: Duration = Duration::from_secs(30);
const AFTER_SAVE: Duration = Duration::from_millis(300);
const LOOP_STEP: f64 = 0.01;

#[derive(Default)]
struct Checks {
    failures: Vec<String>,
}

impl Checks {
    fn expect(&mut self, condition: bool, description: impl Into<String>) {
        if !condition {
            self.failures.push(description.into());
        }
    }
}

fn write(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().expect("a file has a folder")).expect("the folder is created");
    fs::write(path, content).expect("the file is written");
}

fn spin_like_a_pending_quit(limit: Duration, until: impl Fn() -> bool) {
    let started = Instant::now();
    while !until() && started.elapsed() < limit {
        let step = NSDate::dateWithTimeIntervalSinceNow(LOOP_STEP);
        NSRunLoop::currentRunLoop().runMode_beforeDate(unsafe { NSModalPanelRunLoopMode }, &step);
    }
}

fn the_cache_is_saved_while_quitting_waits(checks: &mut Checks) {
    let volume = TempTree::new().expect("a temp volume is created");
    for number in 0..NOTE_FILES {
        write(
            &volume.path().join(format!("Notes/note-{number:02}.txt")),
            format!("note {number}\n").as_bytes(),
        );
    }
    let state = TempTree::new().expect("a temp state is created");
    let paths = SessionPaths::in_directory(state.path());
    let mut session = Session::open(volume.path(), &paths, APP_VERSION).expect("the session opens");
    session.scan(None, CancelToken::default()).expect("the volume scans");
    let nodes = session.index().expect("a scan installs an index").len();
    fs::remove_file(&paths.cache).expect("the cache written by the scan is removed");

    let saved: Rc<RefCell<Vec<io::Result<()>>>> = Rc::default();
    let reported = Rc::clone(&saved);
    let saving = QuitJob::start(session, move |result| reported.borrow_mut().push(result));
    checks.expect(
        saved.borrow().is_empty(),
        "start returns to the main thread before the cache is saved",
    );
    spin_like_a_pending_quit(SAVE_LIMIT, || !saved.borrow().is_empty());
    checks.expect(
        !saved.borrow().is_empty(),
        format!("the save is reported while the run loop waits in the modal panel mode within {SAVE_LIMIT:?}"),
    );
    spin_like_a_pending_quit(AFTER_SAVE, || false);
    let results = saved.borrow();
    checks.expect(
        results.len() == 1,
        format!("the save is reported once, not {}", results.len()),
    );
    if let Some(Err(error)) = results.first() {
        checks.expect(false, format!("the cache is written without an error: {error}"));
    }
    drop(saving);

    let mut reopened = Session::open(volume.path(), &paths, APP_VERSION).expect("the session reopens");
    let opening = reopened.open_cached().expect("the cache reads");
    checks.expect(
        opening == CacheOpening::Opened,
        format!("the saved cache opens: {opening:?}"),
    );
    let reopened_nodes = reopened.index().map(Index::len);
    checks.expect(
        reopened_nodes == Some(nodes),
        format!("the saved cache holds every node: {reopened_nodes:?} of {nodes}"),
    );
}

fn main() -> ExitCode {
    let marker = MainThreadMarker::new().expect("the quit check runs on the main thread");
    let _application = NSApplication::sharedApplication(marker);
    let mut checks = Checks::default();
    the_cache_is_saved_while_quitting_waits(&mut checks);
    if checks.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "saving the cache on quit breaks its contract:\n{}",
            checks.failures.join("\n")
        );
        ExitCode::FAILURE
    }
}
