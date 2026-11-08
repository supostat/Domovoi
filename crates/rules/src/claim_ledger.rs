use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap};
use std::iter;

use domovoi_index::{Index, NodeId, Reclaimable};

use crate::blocked_item::BlockedItem;
use crate::card_item::CardItem;
use crate::deny_reason::DenyReason;
use crate::denylist::Denylist;
use crate::project_card::ProjectCard;
use crate::project_item::ProjectItem;
use crate::recommendations::{LevelTotals, RuleCard};
use crate::rule_set::{Claim, RuleSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Due {
    Nothing,
    Verdict,
    Everything,
}

#[derive(Clone, Debug)]
enum Standing {
    Counted(Reclaimable),
    Blocked(DenyReason),
}

#[derive(Clone, Debug)]
struct LedgerEntry {
    claim: Claim,
    standing: Option<Standing>,
    due: Due,
}

#[derive(Debug, Default)]
pub(crate) struct ClaimLedger {
    entries: Vec<Option<LedgerEntry>>,
    positions: HashMap<NodeId, usize>,
}

impl ClaimLedger {
    pub(crate) fn of_claims(claims: Vec<Claim>) -> Self {
        let mut ledger = Self::default();
        for claim in claims {
            ledger.claim(claim);
        }
        ledger
    }

    pub(crate) fn of_recommendations(cards: Vec<RuleCard>, projects: Vec<ProjectCard>) -> Self {
        let mut ledger = Self::default();
        for card in cards {
            let rule = card.rule;
            let counted = card
                .items
                .into_iter()
                .map(|item| (Claim { node: item.node, rule }, Standing::Counted(item.reclaimable)));
            let blocked = card
                .blocked
                .into_iter()
                .map(|(node, reason)| (Claim { node, rule }, Standing::Blocked(reason)));
            for (claim, standing) in counted.chain(blocked) {
                ledger.settled(claim, standing);
            }
        }
        for project in projects {
            let counted = project.items.into_iter().map(|item| {
                (
                    Claim {
                        node: item.node,
                        rule: item.rule,
                    },
                    Standing::Counted(item.reclaimable),
                )
            });
            let blocked = project.blocked.into_iter().map(|blocked| {
                (
                    Claim {
                        node: blocked.node,
                        rule: blocked.rule,
                    },
                    Standing::Blocked(blocked.reason),
                )
            });
            for (claim, standing) in counted.chain(blocked) {
                ledger.settled(claim, standing);
            }
        }
        ledger
    }

    pub(crate) fn claim(&mut self, claim: Claim) {
        match self.positions.get(&claim.node) {
            Some(position) => {
                let entry = self.entries[*position]
                    .as_mut()
                    .expect("a position points at a live entry");
                entry.claim.rule = claim.rule;
                entry.due = Due::Everything;
            }
            None => self.insert(LedgerEntry {
                claim,
                standing: None,
                due: Due::Everything,
            }),
        }
    }

    pub(crate) fn reassess(&mut self, node: NodeId) {
        if let Some(position) = self.positions.get(&node) {
            self.entries[*position]
                .as_mut()
                .expect("a position points at a live entry")
                .due = Due::Everything;
        }
    }

    pub(crate) fn relocate(&mut self, relocated: &HashMap<NodeId, NodeId>) {
        let moves: Vec<(NodeId, NodeId)> = self
            .positions
            .keys()
            .filter_map(|previous| relocated.get(previous).map(|current| (*previous, *current)))
            .collect();
        for (previous, current) in moves {
            let position = self
                .positions
                .remove(&previous)
                .expect("a relocated claim is in the ledger");
            let entry = self.entries[position]
                .as_mut()
                .expect("a position points at a live entry");
            entry.claim.node = current;
            entry.due = entry.due.max(Due::Verdict);
            self.positions.insert(current, position);
        }
    }

    pub(crate) fn forget(&mut self, node: NodeId) {
        if let Some(position) = self.positions.remove(&node) {
            self.entries[position] = None;
        }
    }

    pub(crate) fn forget_detached(&mut self, index: &Index) {
        let detached: Vec<NodeId> = self
            .positions
            .keys()
            .filter(|node| !index.is_live(**node))
            .copied()
            .collect();
        for node in detached {
            self.forget(node);
        }
    }

    pub(crate) fn forget_within(&mut self, index: &Index, top: NodeId) {
        let within: Vec<NodeId> = self
            .positions
            .keys()
            .filter(|node| ancestry(index, **node).any(|ancestor| ancestor == top))
            .copied()
            .collect();
        for node in within {
            self.forget(node);
        }
    }

    pub(crate) fn holder_of(&self, index: &Index, node: NodeId) -> Option<NodeId> {
        ancestry(index, node).find(|ancestor| self.positions.contains_key(ancestor))
    }

    pub(crate) fn settle(&mut self, index: &Index, denylist: &Denylist) {
        for entry in self.entries.iter_mut().flatten() {
            if entry.due == Due::Nothing {
                continue;
            }
            let node = entry.claim.node;
            entry.standing = Some(match (denylist.verdict(index, node), entry.standing.take()) {
                (Some(reason), _) => Standing::Blocked(reason),
                (None, Some(Standing::Counted(reclaimable))) if entry.due == Due::Verdict => {
                    Standing::Counted(reclaimable)
                }
                (None, _) => Standing::Counted(index.reclaimable_estimate(node)),
            });
            entry.due = Due::Nothing;
        }
    }

    pub(crate) fn into_recommendations(
        self,
        index: &Index,
        rules: &RuleSet,
        running_bundle_ids: &BTreeSet<String>,
    ) -> (Vec<RuleCard>, Vec<ProjectCard>, LevelTotals) {
        let mut cards: Vec<RuleCard> = Vec::new();
        let mut projects: Vec<ProjectCard> = Vec::new();
        let mut project_positions: HashMap<NodeId, usize> = HashMap::new();
        let mut roots: HashMap<NodeId, NodeId> = HashMap::new();
        let mut totals = LevelTotals::default();
        for entry in self.entries.into_iter().flatten() {
            let claim = entry.claim;
            let standing = entry.standing.expect("every claim is settled before its card is built");
            let definition = rules.rule(claim.rule);
            if let Standing::Counted(reclaimable) = &standing
                && !definition.is_info_only()
            {
                totals.add(definition.level, reclaimable.bytes);
            }
            if let Some(marker_folder) = rules.project_folder(index, claim) {
                let root = *roots
                    .entry(marker_folder)
                    .or_insert_with(|| ProjectCard::root_of(index, marker_folder));
                let position = *project_positions.entry(root).or_insert_with(|| {
                    projects.push(ProjectCard::empty(root));
                    projects.len() - 1
                });
                file_into_project(&mut projects[position], claim, standing);
            } else {
                let position = cards
                    .iter()
                    .position(|card| card.rule == claim.rule)
                    .unwrap_or_else(|| {
                        cards.push(empty_card(rules, claim.rule, running_bundle_ids));
                        cards.len() - 1
                    });
                file_into_card(&mut cards[position], claim.node, standing);
            }
        }
        cards.sort_by_key(|card| Reverse(card.reclaimable.bytes));
        for project in &mut projects {
            project.items.sort_by_key(|item| Reverse(item.reclaimable.bytes));
            project.settle_last_work(index);
        }
        projects.sort_by_key(|project| Reverse(project.reclaimable.bytes));
        (cards, projects, totals)
    }

    fn settled(&mut self, claim: Claim, standing: Standing) {
        self.insert(LedgerEntry {
            claim,
            standing: Some(standing),
            due: Due::Nothing,
        });
    }

    fn insert(&mut self, entry: LedgerEntry) {
        self.positions.insert(entry.claim.node, self.entries.len());
        self.entries.push(Some(entry));
    }
}

fn empty_card(rules: &RuleSet, rule: usize, running_bundle_ids: &BTreeSet<String>) -> RuleCard {
    let definition = rules.rule(rule);
    RuleCard {
        rule,
        items: Vec::new(),
        blocked: Vec::new(),
        reclaimable: Reclaimable::default(),
        owner_running: definition
            .owner_bundle
            .is_some_and(|owner| running_bundle_ids.contains(owner)),
    }
}

fn file_into_card(card: &mut RuleCard, node: NodeId, standing: Standing) {
    match standing {
        Standing::Counted(reclaimable) => {
            card.reclaimable += reclaimable;
            card.items.push(CardItem { node, reclaimable });
        }
        Standing::Blocked(reason) => card.blocked.push((node, reason)),
    }
}

fn file_into_project(project: &mut ProjectCard, claim: Claim, standing: Standing) {
    match standing {
        Standing::Counted(reclaimable) => {
            project.reclaimable += reclaimable;
            project.items.push(ProjectItem {
                node: claim.node,
                rule: claim.rule,
                reclaimable,
            });
        }
        Standing::Blocked(reason) => project.blocked.push(BlockedItem {
            node: claim.node,
            rule: claim.rule,
            reason,
        }),
    }
}

pub(crate) fn ancestry(index: &Index, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    iter::successors(Some(node), |current| index.parent(*current))
}
