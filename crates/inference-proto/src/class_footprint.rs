use crate::model_class::ModelClass;

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
