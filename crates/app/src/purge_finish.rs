#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PurgeFinish {
    Completed,
    Stalled,
    TimedOut,
    Cancelled,
}
