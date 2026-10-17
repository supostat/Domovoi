use domovoi_index::Category;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegendEntry {
    pub category: Category,
    pub title: String,
    pub size_text: String,
}
