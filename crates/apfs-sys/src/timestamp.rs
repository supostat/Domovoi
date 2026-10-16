use std::io;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const NANOSECONDS_PER_SECOND: i64 = 1_000_000_000;

pub(crate) fn system_time(seconds: i64, nanoseconds: i64) -> io::Result<SystemTime> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "timespec is out of range");
    if !(0..NANOSECONDS_PER_SECOND).contains(&nanoseconds) {
        return Err(invalid());
    }
    let fraction = Duration::from_nanos(nanoseconds.unsigned_abs());
    let whole = Duration::from_secs(seconds.unsigned_abs());
    let base = if seconds >= 0 {
        UNIX_EPOCH.checked_add(whole)
    } else {
        UNIX_EPOCH.checked_sub(whole)
    };
    base.and_then(|time| time.checked_add(fraction)).ok_or_else(invalid)
}

pub(crate) fn timespec_parts(time: SystemTime) -> (i64, i64) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => (
            i64::try_from(after.as_secs()).expect("time fits in a timespec"),
            i64::from(after.subsec_nanos()),
        ),
        Err(before) => {
            let before = before.duration();
            let seconds = i64::try_from(before.as_secs()).expect("time fits in a timespec");
            match before.subsec_nanos() {
                0 => (-seconds, 0),
                nanoseconds => (-seconds - 1, NANOSECONDS_PER_SECOND - i64::from(nanoseconds)),
            }
        }
    }
}
