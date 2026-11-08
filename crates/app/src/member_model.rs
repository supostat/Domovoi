use domovoi_index::NodeId;
use serde::Deserialize;

use crate::strings::REFUSED_PACKAGE;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberModel {
    pub path: String,
    pub size: u64,
    #[serde(default)]
    pub frees: u64,
    #[serde(default)]
    pub clone: bool,
    #[serde(default)]
    pub checked: bool,
    #[serde(default)]
    pub refusal: Option<String>,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(skip)]
    pub node: Option<NodeId>,
}

impl MemberModel {
    pub fn can_check(&self, clone_mode: bool) -> bool {
        !self.clone && self.package.is_none() && (!clone_mode || self.refusal.is_none())
    }

    pub fn shown_refusal(&self, clone_mode: bool) -> Option<String> {
        match &self.package {
            Some(package) => Some(format!("{REFUSED_PACKAGE} {package}")),
            None => self.refusal.clone().filter(|_| clone_mode),
        }
    }
}
