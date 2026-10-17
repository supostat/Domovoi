use domovoi_index::Category;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramNode {
    pub name: String,
    pub category: Category,
    pub size: u64,
    pub depth: u32,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}
