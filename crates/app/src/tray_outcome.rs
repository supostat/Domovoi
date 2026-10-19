use domovoi_theme::format_size;
use serde::Deserialize;

use crate::strings::{ESTIMATE_MARK, MOVED_TO_TRASH};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrayOutcome {
    pub moved: u64,
    #[serde(default)]
    pub estimated: bool,
    #[serde(default)]
    pub failures: Vec<String>,
}

impl TrayOutcome {
    pub fn moved_line(&self) -> String {
        if self.estimated {
            format!("{MOVED_TO_TRASH} {ESTIMATE_MARK} {}", format_size(self.moved))
        } else {
            format!("{MOVED_TO_TRASH} {}", format_size(self.moved))
        }
    }
}
