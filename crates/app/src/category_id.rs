use domovoi_index::Category;
use serde::{Deserialize, Deserializer};

pub(crate) fn category_from_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Category, D::Error> {
    let id = String::deserialize(deserializer)?;
    Category::ALL
        .into_iter()
        .find(|category| category.id() == id)
        .ok_or_else(|| serde::de::Error::custom(format!("unknown category {id:?}")))
}
