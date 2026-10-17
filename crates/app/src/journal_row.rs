use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalRow {
    pub title: String,
    pub path: String,
    pub size: u64,
    #[serde(skip)]
    pub position: usize,
}
