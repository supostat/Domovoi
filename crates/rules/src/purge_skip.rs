#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PurgeSkip {
    NotInTrash,
    Replaced,
}
