use domovoi_inference_proto::ModelClass;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

const GIBIBYTE: u64 = 1024 * 1024 * 1024;
const COMPUTE_RESERVE: u64 = GIBIBYTE / 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassFootprint {
    pub class: ModelClass,
    pub context_length: u32,
    pub weights_bytes: u64,
    pub cache_bytes_per_token: u64,
}

pub const FOOTPRINTS: [ClassFootprint; 3] = [
    ClassFootprint {
        class: ModelClass::Compact,
        context_length: 8 * 1024,
        weights_bytes: 2_740_000_000,
        cache_bytes_per_token: 78_336,
    },
    ClassFootprint {
        class: ModelClass::Standard,
        context_length: 16 * 1024,
        weights_bytes: 5_030_000_000,
        cache_bytes_per_token: 78_336,
    },
    ClassFootprint {
        class: ModelClass::Extended,
        context_length: 32 * 1024,
        weights_bytes: 9_000_000_000,
        cache_bytes_per_token: 87_040,
    },
];

impl ClassFootprint {
    pub fn of(class: ModelClass) -> Self {
        FOOTPRINTS
            .into_iter()
            .find(|footprint| footprint.class == class)
            .expect("every class has a footprint")
    }

    pub fn required_bytes(self) -> u64 {
        self.weights_bytes + self.cache_bytes_per_token * u64::from(self.context_length) + COMPUTE_RESERVE
    }
}

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
    use domovoi_inference_proto::ModelClass;

    use super::{ClassFootprint, MemoryBudget};

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
