use crate::purge_finish::PurgeFinish;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PurgeReport {
    pub estimate: u64,
    pub freed: u64,
    pub available_before: u64,
    pub available_after: u64,
    pub finish: PurgeFinish,
}
