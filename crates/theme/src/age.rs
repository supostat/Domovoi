use std::time::{SystemTime, UNIX_EPOCH};

use objc2_foundation::{NSDate, NSTimeZone};

use crate::digits::NO_BREAK_SPACE;
use crate::plural::PluralForms;

const SECONDS_PER_DAY: f64 = 86_400.0;
const MONTHS_BEFORE_COUNTING_YEARS: u64 = 24;
const DAYS: PluralForms = PluralForms {
    one: "день",
    few: "дня",
    many: "дней",
};
const MONTHS: PluralForms = PluralForms {
    one: "месяц",
    few: "месяца",
    many: "месяцев",
};
const YEARS: PluralForms = PluralForms {
    one: "год",
    few: "года",
    many: "лет",
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateSource {
    LastOpened,
    LastOpenedBySpotlight,
    LastOpenedInherited,
    Modified,
    Added,
}

impl DateSource {
    const fn label(self) -> &'static str {
        match self {
            Self::LastOpened => "последнее обращение",
            Self::LastOpenedBySpotlight => "последнее обращение по Spotlight",
            Self::LastOpenedInherited => "последнее обращение к оригиналу",
            Self::Modified => "изменён",
            Self::Added => "появился здесь",
        }
    }
}

pub fn format_age(then: SystemTime, now: SystemTime, source: DateSource) -> String {
    format!("{} {}", source.label(), age(LocalDate::of(then), LocalDate::of(now)))
}

fn age(then: LocalDate, now: LocalDate) -> String {
    let Ok(days) = u64::try_from(now.day_number - then.day_number) else {
        return "в будущем".to_owned();
    };
    match days {
        0 => "сегодня".to_owned(),
        1 => "вчера".to_owned(),
        _ => match then.whole_months_until(now) {
            0 => ago(days, &DAYS),
            months if months < MONTHS_BEFORE_COUNTING_YEARS => ago(months, &MONTHS),
            months => ago(months / 12, &YEARS),
        },
    }
}

fn ago(count: u64, forms: &PluralForms) -> String {
    format!("{count}{NO_BREAK_SPACE}{} назад", forms.select(count))
}

#[derive(Clone, Copy, Debug)]
struct LocalDate {
    day_number: i64,
    year: i64,
    month: i64,
    day: i64,
}

impl LocalDate {
    fn of(time: SystemTime) -> Self {
        let unix_seconds = match time.duration_since(UNIX_EPOCH) {
            Ok(after_epoch) => after_epoch.as_secs_f64(),
            Err(before_epoch) => -before_epoch.duration().as_secs_f64(),
        };
        let instant = NSDate::dateWithTimeIntervalSince1970(unix_seconds);
        let utc_offset = NSTimeZone::localTimeZone().secondsFromGMTForDate(&instant) as f64;
        Self::from_day_number(((unix_seconds + utc_offset) / SECONDS_PER_DAY).floor() as i64)
    }

    fn from_day_number(day_number: i64) -> Self {
        let shifted = day_number + 719_468;
        let era = shifted.div_euclid(146_097);
        let day_of_era = shifted.rem_euclid(146_097);
        let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_from_march = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
        let month = if month_from_march < 10 {
            month_from_march + 3
        } else {
            month_from_march - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Self {
            day_number,
            year,
            month,
            day,
        }
    }

    fn whole_months_until(self, later: Self) -> u64 {
        let months = (later.year - self.year) * 12 + (later.month - self.month) - i64::from(later.day < self.day);
        u64::try_from(months).expect("a later date is a non-negative number of months after an earlier one")
    }
}
