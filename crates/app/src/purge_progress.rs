#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurgeProgress {
    pub estimate: u64,
    pub freed: u64,
}
