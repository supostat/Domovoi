use std::ffi::CStr;
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use domovoi_log_sys::SystemLog;
use domovoi_testkit::test_timeout;

const SUBSYSTEM: &CStr = c"com.domovoi.test";
const CATEGORY: &CStr = c"unified-log";
const POLL: Duration = Duration::from_millis(250);

fn unique_marker() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_nanos();
    format!("domovoi-log-sys-{}-{nanos}", process::id())
}

fn shown(marker: &str) -> String {
    let predicate = format!(
        "processID == {} AND subsystem == \"{}\" AND eventMessage CONTAINS \"{marker}\"",
        process::id(),
        SUBSYSTEM.to_str().expect("the subsystem is UTF-8"),
    );
    let output = Command::new("/usr/bin/log")
        .args(["show", "--last", "1m", "--style", "compact", "--predicate", &predicate])
        .output()
        .expect("log show runs");
    assert!(
        output.status.success(),
        "log show failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn an_error_and_a_notice_reach_the_unified_log_whole() {
    let marker = unique_marker();
    let log = SystemLog::new(SUBSYSTEM, CATEGORY);
    log.error(&format!("{marker} error before\0after"));
    log.notice(&format!("{marker} notice"));
    let expected = [
        format!("{marker} error before\u{2400}after"),
        format!("{marker} notice"),
    ];
    let deadline = Instant::now() + test_timeout();
    loop {
        let records = shown(&marker);
        if expected.iter().all(|line| records.contains(line.as_str())) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the unified log did not show {expected:?}; it showed:\n{records}"
        );
        thread::sleep(POLL);
    }
}
