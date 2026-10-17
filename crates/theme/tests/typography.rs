use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domovoi_theme::{DateSource, FONT_STACKS, PluralForms, format_age, format_count, format_memory, format_size};
use objc2_foundation::{NSDate, NSTimeZone};

const FILES: PluralForms = PluralForms {
    one: "файл",
    few: "файла",
    many: "файлов",
};
const SECONDS_PER_DAY: i64 = 86_400;

fn no_break(text: &str) -> String {
    text.replace(' ', "\u{A0}")
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn local_time(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> SystemTime {
    let wall_clock_seconds = days_from_civil(year, month, day) * SECONDS_PER_DAY + hour * 3_600 + minute * 60;
    let utc_offset = NSTimeZone::localTimeZone()
        .secondsFromGMTForDate(&NSDate::dateWithTimeIntervalSince1970(wall_clock_seconds as f64))
        as i64;
    let unix_seconds = u64::try_from(wall_clock_seconds - utc_offset).expect("test dates are after the epoch");
    UNIX_EPOCH + Duration::from_secs(unix_seconds)
}

fn local_noon(year: i64, month: i64, day: i64) -> SystemTime {
    local_time(year, month, day, 12, 0)
}

fn fonts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts")
}

fn manifest_entries(manifest: &str) -> Vec<BTreeMap<String, String>> {
    manifest
        .split("[[font]]")
        .skip(1)
        .map(|entry| {
            entry
                .lines()
                .filter_map(|line| line.split_once(" = "))
                .map(|(key, value)| (key.trim().to_owned(), value.trim().trim_matches('"').to_owned()))
                .collect()
        })
        .collect()
}

fn sha256(path: &Path) -> String {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("shasum runs");
    assert!(output.status.success(), "shasum failed for {}", path.display());
    String::from_utf8(output.stdout)
        .expect("shasum prints ASCII")
        .split_whitespace()
        .next()
        .expect("shasum prints a digest")
        .to_owned()
}

#[test]
fn sizes_use_decimal_units_a_comma_and_a_no_break_space() {
    assert_eq!(format_size(222_100_000_000), no_break("222,1 ГБ"));
    assert_eq!(format_size(38_200_000_000), no_break("38,2 ГБ"));
    assert_eq!(format_size(46_000_000), no_break("46 МБ"));
    assert_eq!(format_size(1_234_560_000_000), no_break("1 234,6 ГБ"));
}

#[test]
fn memory_counts_whole_binary_gigabytes_as_macos_names_them() {
    assert_eq!(format_memory(8 * 1024 * 1024 * 1024), no_break("8 ГБ"));
    assert_eq!(format_memory(34_359_738_368), no_break("32 ГБ"));
    assert_eq!(format_memory(17_179_869_184 - 1), no_break("16 ГБ"));
}

#[test]
fn small_sizes_step_down_to_kilobytes_and_bytes() {
    assert_eq!(format_size(0), no_break("0 Б"));
    assert_eq!(format_size(512), no_break("512 Б"));
    assert_eq!(format_size(12_300), no_break("12 КБ"));
}

#[test]
fn rounding_carries_into_the_next_unit() {
    assert_eq!(format_size(999_999), no_break("1 МБ"));
    assert_eq!(format_size(999_600_000), no_break("1,0 ГБ"));
    assert_eq!(format_size(38_250_000_000), no_break("38,3 ГБ"));
}

#[test]
fn counts_group_digits_and_agree_with_the_number() {
    assert_eq!(format_count(1_284_112, &FILES), no_break("1 284 112 файлов"));
    let agreements = [
        (1, "1 файл"),
        (2, "2 файла"),
        (4, "4 файла"),
        (5, "5 файлов"),
        (11, "11 файлов"),
        (12, "12 файлов"),
        (21, "21 файл"),
        (22, "22 файла"),
        (111, "111 файлов"),
        (1_000, "1 000 файлов"),
    ];
    for (count, expected) in agreements {
        assert_eq!(format_count(count, &FILES), no_break(expected));
    }
}

#[test]
fn ages_count_calendar_days_months_and_years() {
    let now = local_noon(2026, 10, 16);
    let ages = [
        (local_noon(2026, 10, 16), "сегодня"),
        (local_noon(2026, 10, 15), "вчера"),
        (local_noon(2026, 10, 14), "2\u{A0}дня назад"),
        (local_noon(2026, 10, 11), "5\u{A0}дней назад"),
        (local_noon(2026, 9, 25), "21\u{A0}день назад"),
        (local_noon(2026, 9, 17), "29\u{A0}дней назад"),
        (local_noon(2026, 9, 16), "1\u{A0}месяц назад"),
        (local_noon(2025, 8, 16), "14\u{A0}месяцев назад"),
        (local_noon(2024, 10, 17), "23\u{A0}месяца назад"),
        (local_noon(2024, 10, 16), "2\u{A0}года назад"),
        (local_noon(2021, 10, 16), "5\u{A0}лет назад"),
        (local_noon(2005, 10, 16), "21\u{A0}год назад"),
        (local_noon(2026, 10, 17), "в будущем"),
    ];
    for (then, expected) in ages {
        assert_eq!(
            format_age(then, now, DateSource::Modified),
            format!("изменён {expected}")
        );
    }
}

#[test]
fn ages_follow_the_local_calendar_not_elapsed_hours() {
    let just_after_midnight = local_time(2026, 10, 16, 0, 30);
    let late_yesterday = local_time(2026, 10, 15, 23, 30);
    assert_eq!(
        format_age(late_yesterday, just_after_midnight, DateSource::LastOpened),
        "последнее обращение вчера"
    );
}

#[test]
fn ages_name_the_source_of_the_date() {
    let now = local_noon(2026, 10, 16);
    let then = local_noon(2025, 8, 16);
    let labelled = [
        (DateSource::LastOpened, "последнее обращение"),
        (DateSource::LastOpenedBySpotlight, "последнее обращение по Spotlight"),
        (DateSource::LastOpenedInherited, "последнее обращение к оригиналу"),
        (DateSource::Modified, "изменён"),
        (DateSource::Added, "появился здесь"),
    ];
    for (source, label) in labelled {
        assert_eq!(format_age(then, now, source), format!("{label} 14\u{A0}месяцев назад"));
    }
}

#[test]
fn bundled_fonts_match_the_manifest_and_the_font_stacks() {
    let fonts_dir = fonts_dir();
    let manifest = std::fs::read_to_string(fonts_dir.join("fonts.toml")).expect("font manifest exists");
    let entries = manifest_entries(&manifest);
    for entry in &entries {
        for (file_key, digest_key) in [("file", "sha256"), ("license_file", "license_sha256")] {
            let path = fonts_dir.join(&entry[file_key]);
            assert_eq!(sha256(&path), entry[digest_key], "{}", path.display());
        }
        assert_eq!(entry["license"], "OFL-1.1");
    }
    let families: Vec<&str> = entries.iter().map(|entry| entry["family"].as_str()).collect();
    assert_eq!(families, [FONT_STACKS.ui[0], FONT_STACKS.mono[0]]);
}
