use domovoi_index::Category;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramOutline {
    pub name: String,
    pub category: Category,
    pub size: u64,
    pub children: Vec<DiagramOutline>,
}
