use domovoi_index::Reclaimable;

use crate::measure_progress::MeasureProgress;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayMeasured {
    pub sizes: Vec<Reclaimable>,
    pub walked: MeasureProgress,
}
