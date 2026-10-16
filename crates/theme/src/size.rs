use crate::digits::{NO_BREAK_SPACE, group_thousands};

const SMALL_UNITS: [(u64, &str); 3] = [(1, "Б"), (1_000, "КБ"), (1_000_000, "МБ")];
const NEXT_UNIT_AT: u64 = 1_000;
const GIGABYTE_TENTH: u64 = 100_000_000;

pub fn format_size(bytes: u64) -> String {
    for (scale, unit) in SMALL_UNITS {
        let rounded = rounded_quotient(bytes, scale);
        if rounded < NEXT_UNIT_AT {
            return format!("{rounded}{NO_BREAK_SPACE}{unit}");
        }
    }
    let tenths = rounded_quotient(bytes, GIGABYTE_TENTH);
    format!("{},{}{NO_BREAK_SPACE}ГБ", group_thousands(tenths / 10), tenths % 10)
}

fn rounded_quotient(value: u64, divisor: u64) -> u64 {
    value / divisor + u64::from((value % divisor) * 2 >= divisor)
}
