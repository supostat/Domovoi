use serde::Deserialize;

use crate::group_model::GroupModel;
use crate::search_stage::SearchStage;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", deny_unknown_fields)]
pub enum DuplicatesState {
    Idle,
    Searching {
        stage: SearchStage,
        done: u64,
        total: u64,
    },
    Found {
        candidates: u64,
        seconds: u64,
        groups: Vec<GroupModel>,
    },
}
