#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PluralForms {
    pub one: &'static str,
    pub few: &'static str,
    pub many: &'static str,
}

impl PluralForms {
    pub const fn select(&self, count: u64) -> &'static str {
        let last_digit = count % 10;
        let last_two_digits = count % 100;
        if last_digit == 1 && last_two_digits != 11 {
            self.one
        } else if matches!(last_digit, 2..=4) && !matches!(last_two_digits, 12..=14) {
            self.few
        } else {
            self.many
        }
    }
}
