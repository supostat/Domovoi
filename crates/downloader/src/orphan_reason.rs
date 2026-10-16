#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrphanReason {
    NotInCatalog,
    DigestMismatch,
}
