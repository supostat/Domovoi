use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PurgeFinish {
    Completed,
    Stalled,
    TimedOut,
    Cancelled,
}
