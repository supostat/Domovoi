use std::time::Duration;

use domovoi_theme::NO_BREAK_SPACE;

use crate::strings::{MINUTES_SHORT, SCAN_FINISHED_IN, SECONDS_SHORT};

const SECONDS_PER_MINUTE: u64 = 60;

pub fn scan_finished_line(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    let minutes = seconds / SECONDS_PER_MINUTE;
    let rest = seconds % SECONDS_PER_MINUTE;
    if minutes == 0 {
        format!("{SCAN_FINISHED_IN} {rest}{NO_BREAK_SPACE}{SECONDS_SHORT}")
    } else {
        format!("{SCAN_FINISHED_IN} {minutes}{NO_BREAK_SPACE}{MINUTES_SHORT} {rest}{NO_BREAK_SPACE}{SECONDS_SHORT}")
    }
}
