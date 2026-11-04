use domovoi_theme::{format_size, group_thousands};

use crate::strings::{ESTIMATE_MARK, TRAY};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraySummary {
    pub objects: usize,
    pub bytes: u64,
}

impl TraySummary {
    pub fn title(&self) -> String {
        if self.objects == 0 {
            return TRAY.to_owned();
        }
        format!(
            "{TRAY} · {} · {ESTIMATE_MARK} {}",
            group_thousands(self.objects as u64),
            format_size(self.bytes)
        )
    }
}
