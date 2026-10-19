use std::path::Path;

use domovoi_index::Index;
use domovoi_rules::{Denylist, LevelTotals, RuleSet};

#[derive(Clone, Copy, Debug)]
pub struct AgentContext<'a> {
    pub index: &'a Index,
    pub rules: &'a RuleSet,
    pub denylist: &'a Denylist,
    pub totals: LevelTotals,
    pub home: &'a Path,
}
