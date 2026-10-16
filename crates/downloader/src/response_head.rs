#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResponseHead {
    pub(crate) status: u16,
    pub(crate) location: Option<String>,
    pub(crate) etag: Option<String>,
    pub(crate) content_range: Option<String>,
    pub(crate) repo_commit: Option<String>,
    pub(crate) linked_etag: Option<String>,
    pub(crate) linked_size: Option<String>,
}
