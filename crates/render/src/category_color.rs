use domovoi_index::Category;
use domovoi_theme::{Palette, Rgba};

pub fn category_color(palette: &Palette, category: Category) -> Rgba {
    match category {
        Category::Apps => palette.c_apps,
        Category::Dev => palette.c_dev,
        Category::Media => palette.c_media,
        Category::Docs => palette.c_docs,
        Category::Cache => palette.c_cache,
        Category::System => palette.c_system,
        Category::Hidden => palette.c_hidden,
        Category::Model => palette.c_model,
        Category::Home => palette.c_home,
    }
}

pub(crate) fn faded(color: Rgba, opacity: f32) -> [f32; 4] {
    color.premultiplied().map(|channel| channel * opacity)
}
