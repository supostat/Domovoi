use std::time::SystemTime;

use domovoi_index::{Index, NodeId, NodeKind, Timestamp};
use domovoi_theme::{DateSource, format_age, format_count};

use crate::strings::{INSIDE, OBJECTS};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexFacts {
    pub created: Option<String>,
    pub modified: Option<String>,
    pub inside: Option<String>,
}

impl IndexFacts {
    pub fn of(index: &Index, id: NodeId, now: SystemTime) -> Self {
        let record = index.node(id);
        let age = |stamp: Timestamp, source: DateSource| stamp.system_time().map(|then| format_age(then, now, source));
        Self {
            created: age(record.created, DateSource::Created),
            modified: age(record.modified, DateSource::Modified),
            inside: (record.kind == NodeKind::Directory)
                .then(|| format!("{INSIDE} {}", format_count(u64::from(record.files), &OBJECTS))),
        }
    }
}
