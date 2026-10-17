#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DuplicateSummary {
    pub candidates: u64,
    pub seconds: u64,
}
