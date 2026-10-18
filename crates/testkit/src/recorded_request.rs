#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedRequest {
    pub path: String,
    pub range: Option<String>,
    pub if_range: Option<String>,
    pub status: u16,
}
