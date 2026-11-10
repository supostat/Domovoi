use serde::Deserialize;

use crate::tray_target::TrayTarget;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRow {
    pub name: String,
    pub path: String,
    pub artifacts: Vec<String>,
    pub age: String,
    pub stale: bool,
    pub amount: u64,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub in_joint_hold: bool,
    #[serde(skip)]
    pub targets: Vec<TrayTarget>,
}

impl ProjectRow {
    pub const STALE_GROUP: usize = 0;
    pub const FRESH_GROUP: usize = 1;

    pub fn selectable(&self) -> bool {
        self.amount > 0 || self.in_joint_hold
    }

    pub fn group(&self) -> usize {
        if self.stale {
            Self::STALE_GROUP
        } else {
            Self::FRESH_GROUP
        }
    }
}
