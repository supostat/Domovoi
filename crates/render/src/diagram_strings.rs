use domovoi_index::Category;
use domovoi_theme::{PluralForms, format_count, format_size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagramStrings {
    pub category_names: [&'static str; 9],
    pub used: &'static str,
    pub up: &'static str,
    pub small_group: &'static str,
    pub objects: PluralForms,
}

impl DiagramStrings {
    pub fn category_name(&self, category: Category) -> &'static str {
        let position = Category::ALL
            .iter()
            .position(|candidate| *candidate == category)
            .expect("every category is listed");
        self.category_names[position]
    }

    pub fn small_group_count(&self, count: usize) -> String {
        format_count(count as u64, &self.objects)
    }

    pub fn small_group_label(&self, count: usize, bytes: u64) -> String {
        format!(
            "{}, {}, {}",
            self.small_group,
            format_size(bytes),
            self.small_group_count(count)
        )
    }
}
