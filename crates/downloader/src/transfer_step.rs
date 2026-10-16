#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TransferStep {
    Complete,
    Restart,
    Forbidden,
    Interrupted(String),
}
