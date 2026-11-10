use crate::measure_progress::MeasureProgress;
use crate::tray_sizes::TraySizes;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayMeasured {
    pub sizes: TraySizes,
    pub walked: MeasureProgress,
}
