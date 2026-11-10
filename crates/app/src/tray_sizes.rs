use domovoi_index::Reclaimable;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TraySizes {
    pub items: Vec<Reclaimable>,
    pub together: Reclaimable,
}
