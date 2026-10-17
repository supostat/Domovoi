use domovoi_index::Category;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagramStrings {
    pub category_names: [&'static str; 9],
    pub used: &'static str,
    pub up: &'static str,
}

impl DiagramStrings {
    pub fn category_name(&self, category: Category) -> &'static str {
        let position = Category::ALL
            .iter()
            .position(|candidate| *candidate == category)
            .expect("every category is listed");
        self.category_names[position]
    }
}
