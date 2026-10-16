use crate::digits::{NO_BREAK_SPACE, group_thousands};
use crate::plural::PluralForms;

pub fn format_count(count: u64, forms: &PluralForms) -> String {
    format!("{}{NO_BREAK_SPACE}{}", group_thousands(count), forms.select(count))
}
