use crate::digits::{NO_BREAK_SPACE, group_thousands};

const GIBIBYTE: u64 = 1024 * 1024 * 1024;

pub fn format_memory(bytes: u64) -> String {
    let gibibytes = bytes / GIBIBYTE + u64::from((bytes % GIBIBYTE) * 2 >= GIBIBYTE);
    format!("{}{NO_BREAK_SPACE}ГБ", group_thousands(gibibytes))
}
