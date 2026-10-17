use domovoi_index::Category;

#[derive(Clone, Debug, PartialEq)]
pub struct LevelRow {
    pub node: usize,
    pub name: String,
    pub size_text: String,
    pub share: f64,
    pub category: Category,
}
