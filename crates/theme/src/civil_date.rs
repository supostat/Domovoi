#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CivilDate {
    pub(crate) year: i64,
    pub(crate) month: i64,
    pub(crate) day: i64,
}

impl CivilDate {
    pub(crate) fn from_day_number(day_number: i64) -> Self {
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
        Self { year, month, day }
    }
}
