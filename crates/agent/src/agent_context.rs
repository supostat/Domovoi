use std::path::Path;

use domovoi_index::Index;
use domovoi_rules::{Denylist, LevelTotals, RuleSet};

use crate::focus_context::FocusContext;
use crate::past_turn::PastTurn;

#[derive(Clone, Copy, Debug)]
pub struct AgentContext<'a> {
    pub index: &'a Index,
    pub rules: &'a RuleSet,
    pub denylist: &'a Denylist,
    pub totals: LevelTotals,
    pub home: &'a Path,
    pub history: &'a [PastTurn],
    pub focus: &'a FocusContext,
}
