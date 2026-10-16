#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StopRequest {
    Pause,
    Cancel,
}
