use domovoi_theme::{format_count, format_size};
use serde::Deserialize;

use crate::member_model::MemberModel;
use crate::strings::{COPIES, EACH, FREES};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupModel {
    pub name: String,
    pub keep: MemberModel,
    pub copies: Vec<MemberModel>,
}

impl GroupModel {
    pub fn frees(&self) -> u64 {
        self.copies
            .iter()
            .filter(|copy| !copy.clone)
            .map(|copy| copy.frees)
            .sum()
    }

    pub fn summary(&self) -> String {
        let members = self.copies.len() as u64 + 1;
        format!(
            "{} · {} {EACH} · {FREES} {}",
            format_count(members, &COPIES),
            format_size(self.keep.size),
            format_size(self.frees())
        )
    }
}
