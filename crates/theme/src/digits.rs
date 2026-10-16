pub(crate) const NO_BREAK_SPACE: char = '\u{A0}';

pub(crate) fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() * 2);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(NO_BREAK_SPACE);
        }
        grouped.push(digit);
    }
    grouped
}
