use domovoi_index::Reclaimable;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TraySizes {
    pub items: Vec<Reclaimable>,
    pub together: Reclaimable,
}

impl TraySizes {
    pub fn freed_together(&self) -> u64 {
        let freed_apart: u64 = self.items.iter().map(|size| size.bytes).sum();
        self.together
            .bytes
            .checked_sub(freed_apart)
            .expect("items deleted together free at least what each frees alone")
    }
}
