use std::time::{SystemTime, UNIX_EPOCH};

use crate::civil_date::CivilDate;

const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;

pub fn format_iso_utc(time: SystemTime) -> String {
    let unix_seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(after_epoch) => i64::try_from(after_epoch.as_secs()).unwrap_or(i64::MAX),
        Err(before_epoch) => -i64::try_from(before_epoch.duration().as_secs()).unwrap_or(i64::MAX),
    };
    let date = CivilDate::from_day_number(unix_seconds.div_euclid(SECONDS_PER_DAY));
    let second_of_day = unix_seconds.rem_euclid(SECONDS_PER_DAY);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        date.year,
        date.month,
        date.day,
        second_of_day / SECONDS_PER_HOUR,
        second_of_day % SECONDS_PER_HOUR / SECONDS_PER_MINUTE,
        second_of_day % SECONDS_PER_MINUTE
    )
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::format_iso_utc;

    #[test]
    fn prints_the_utc_instant_with_a_zulu_suffix() {
        assert_eq!(format_iso_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(
            format_iso_utc(UNIX_EPOCH + Duration::from_secs(1_793_095_200)),
            "2026-10-27T10:00:00Z"
        );
        assert_eq!(
            format_iso_utc(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
        assert_eq!(
            format_iso_utc(UNIX_EPOCH - Duration::from_secs(1)),
            "1969-12-31T23:59:59Z"
        );
    }
}
