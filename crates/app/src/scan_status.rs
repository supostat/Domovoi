use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanStatus {
    pub bytes: u64,
    pub files: u64,
    pub sweep: f32,
    pub threads: usize,
    pub current: String,
    #[serde(default)]
    pub full_first_scan: bool,
}
