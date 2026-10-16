#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Bucket {
    pub(crate) size: u64,
    pub(crate) fully_hashed: bool,
    pub(crate) members: Vec<usize>,
}
