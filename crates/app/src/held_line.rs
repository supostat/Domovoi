use domovoi_theme::format_size;

use crate::strings::{HELD_BY_CLONES, HELD_BY_HARD_LINKS, HELD_BY_SNAPSHOTS, HELD_PREFIX};

pub(crate) fn held_line(held_by_snapshots: u64, held_by_clones: u64, held_by_hard_links: u64) -> Option<String> {
    let parts: Vec<String> = [
        (held_by_snapshots, HELD_BY_SNAPSHOTS),
        (held_by_clones, HELD_BY_CLONES),
        (held_by_hard_links, HELD_BY_HARD_LINKS),
    ]
    .into_iter()
    .filter(|(bytes, _)| *bytes > 0)
    .map(|(bytes, holder)| format!("{} {holder}", format_size(bytes)))
    .collect();
    (!parts.is_empty()).then(|| format!("{HELD_PREFIX} {}", parts.join(" · ")))
}
