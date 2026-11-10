use std::collections::{BTreeMap, HashMap};

use domovoi_index::{HeldGroup, HeldKey, NodeId};

use crate::project_card::ProjectCard;
use crate::recommendations::RuleCard;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JointHold {
    pub items: Vec<NodeId>,
    pub bytes: u64,
}

#[derive(Default)]
struct SharedGroup {
    members_inside: u32,
    members_total: u32,
    bytes: u64,
    items: Vec<NodeId>,
}

impl JointHold {
    pub(crate) fn gather(cards: &[RuleCard], projects: &[ProjectCard]) -> Vec<Self> {
        let held_by_items = cards
            .iter()
            .flat_map(|card| card.items.iter().map(|item| (item.node, &item.held)))
            .chain(
                projects
                    .iter()
                    .flat_map(|project| project.items.iter().map(|item| (item.node, &item.held))),
            );
        let mut groups: HashMap<HeldKey, SharedGroup> = HashMap::new();
        for (item, held) in held_by_items {
            for group in held {
                groups.entry(group.key).or_default().add(item, group);
            }
        }
        let mut joint: BTreeMap<Vec<NodeId>, u64> = BTreeMap::new();
        for mut group in groups.into_values() {
            if group.members_inside >= group.members_total {
                group.items.sort_unstable();
                *joint.entry(group.items).or_default() += group.bytes;
            }
        }
        joint.into_iter().map(|(items, bytes)| Self { items, bytes }).collect()
    }
}

impl SharedGroup {
    fn add(&mut self, item: NodeId, group: &HeldGroup) {
        self.members_inside += group.members_inside;
        self.members_total = self.members_total.max(group.members_total);
        self.bytes = self.bytes.max(group.bytes);
        self.items.push(item);
    }
}
