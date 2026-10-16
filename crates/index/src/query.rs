use std::path::PathBuf;
use std::time::SystemTime;

use crate::category::Category;
use crate::index::Index;
use crate::node_id::NodeId;
use crate::safety_level::{SafetyLevel, SafetyLevels};
use crate::timestamp::Timestamp;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub category: Option<Category>,
    pub level: Option<SafetyLevel>,
    pub min_allocated: u64,
    pub inactive_since: Option<SystemTime>,
    pub path_prefix: Option<PathBuf>,
}

impl Index {
    pub fn query(&self, query: &Query, levels: &dyn SafetyLevels) -> Vec<NodeId> {
        let start = match &query.path_prefix {
            Some(prefix) => match self.find(prefix) {
                Some(node) => vec![node],
                None => return Vec::new(),
            },
            None => self.children(self.root()).collect(),
        };
        let threshold = query.inactive_since.map(Timestamp::from_system_time);
        let mut pending = start;
        let mut matches = Vec::new();
        while let Some(node) = pending.pop() {
            if self.node(node).allocated < query.min_allocated {
                continue;
            }
            if self.matches(node, query, threshold, levels) {
                matches.push(node);
            } else {
                pending.extend(self.children(node));
            }
        }
        matches.sort_by_key(|node| std::cmp::Reverse(self.node(*node).allocated));
        matches
    }

    pub fn last_activity(&self, id: NodeId) -> Timestamp {
        let node = self.node(id);
        let owner_opened = self.app(node.app).map_or(Timestamp::UNKNOWN, |app| app.last_opened);
        node.newest_opened.max(node.newest_modified).max(owner_opened)
    }

    fn matches(&self, node: NodeId, query: &Query, threshold: Option<Timestamp>, levels: &dyn SafetyLevels) -> bool {
        let record = self.node(node);
        query.category.is_none_or(|category| record.category == category)
            && query.level.is_none_or(|level| levels.level(self, node) == Some(level))
            && threshold.is_none_or(|threshold| {
                let activity = self.last_activity(node);
                activity.is_known() && activity < threshold
            })
    }
}
