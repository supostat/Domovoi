#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Bucket {
    pub(crate) size: u64,
    pub(crate) members: Vec<usize>,
}
