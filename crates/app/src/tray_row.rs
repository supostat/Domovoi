use domovoi_index::Category;
use serde::Deserialize;

use crate::category_id::category_from_id;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrayRow {
    #[serde(deserialize_with = "category_from_id")]
    pub category: Category,
    pub title: String,
    pub size: u64,
}
