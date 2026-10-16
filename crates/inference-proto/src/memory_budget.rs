use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

use crate::class_footprint::ClassFootprint;
use crate::model_class::ModelClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryBudget {
    pub working_set_bytes: u64,
}

impl MemoryBudget {
    pub fn of_default_device() -> Option<Self> {
        MTLCreateSystemDefaultDevice().map(|device| Self {
            working_set_bytes: device.recommendedMaxWorkingSetSize(),
        })
    }

    pub fn fits(self, class: ModelClass) -> bool {
        ClassFootprint::of(class).required_bytes() <= self.working_set_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryBudget;
    use crate::class_footprint::ClassFootprint;
    use crate::model_class::ModelClass;

    const GIBIBYTE: u64 = 1024 * 1024 * 1024;
    const LARGEST_SCAN_INDEX_BYTES: u64 = 900_000_000;

    fn working_set_of(memory_gibibytes: u64) -> MemoryBudget {
        MemoryBudget {
            working_set_bytes: memory_gibibytes * GIBIBYTE * 74 / 100,
        }
    }

    #[test]
    fn each_class_fits_the_machines_it_is_offered_to() {
        assert!(working_set_of(8).fits(ModelClass::Compact));
        assert!(working_set_of(16).fits(ModelClass::Standard));
        assert!(working_set_of(32).fits(ModelClass::Extended));
    }

    #[test]
    fn the_compact_class_leaves_room_for_the_largest_scan_index_on_eight_gigabytes() {
        let required = ClassFootprint::of(ModelClass::Compact).required_bytes() + LARGEST_SCAN_INDEX_BYTES;
        assert!(required <= working_set_of(8).working_set_bytes);
    }
}
