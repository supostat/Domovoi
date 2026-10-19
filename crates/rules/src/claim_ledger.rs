use std::collections::{BTreeSet, HashMap};
use std::iter;

use domovoi_index::{Index, NodeId, Reclaimable, SafetyLevel};

use crate::card_item::CardItem;
use crate::deny_reason::DenyReason;
use crate::denylist::Denylist;
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

    pub(crate) fn of_cards(cards: Vec<RuleCard>) -> Self {
        let mut ledger = Self::default();
        for card in cards {
            let rule = card.rule;
            let counted = card
                .items
                .into_iter()
                .map(|item| (item.node, Standing::Counted(item.reclaimable)));
            let blocked = card
                .blocked
                .into_iter()
                .map(|(node, reason)| (node, Standing::Blocked(reason)));
            for (node, standing) in counted.chain(blocked) {
                ledger.insert(LedgerEntry {
                    claim: Claim { node, rule },
                    standing: Some(standing),
                    due: Due::Nothing,
                });
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

    pub(crate) fn into_cards(
        self,
        rules: &RuleSet,
        running_bundle_ids: &BTreeSet<String>,
    ) -> (Vec<RuleCard>, LevelTotals) {
        let mut cards: Vec<RuleCard> = Vec::new();
        for entry in self.entries.into_iter().flatten() {
            let Claim { node, rule } = entry.claim;
            let position = if let Some(position) = cards.iter().position(|card| card.rule == rule) {
                position
            } else {
                let definition = rules.rule(rule);
                cards.push(RuleCard {
                    rule,
                    items: Vec::new(),
                    blocked: Vec::new(),
                    reclaimable: Reclaimable::default(),
                    owner_running: definition
                        .owner_bundle
                        .is_some_and(|owner| running_bundle_ids.contains(owner)),
                    preselected: definition.level == SafetyLevel::Safe && !definition.is_info_only(),
                });
                cards.len() - 1
            };
            let card = &mut cards[position];
            match entry.standing.expect("every claim is settled before its card is built") {
                Standing::Counted(reclaimable) => {
                    card.reclaimable += reclaimable;
                    card.items.push(CardItem { node, reclaimable });
                }
                Standing::Blocked(reason) => card.blocked.push((node, reason)),
            }
        }
        let mut totals = LevelTotals::default();
        for card in &cards {
            let rule = rules.rule(card.rule);
            if !rule.is_info_only() {
                totals.add(rule.level, card.reclaimable.bytes);
            }
        }
        cards.sort_by_key(|card| std::cmp::Reverse(card.reclaimable.bytes));
        (cards, totals)
    }

    fn insert(&mut self, entry: LedgerEntry) {
        self.positions.insert(entry.claim.node, self.entries.len());
        self.entries.push(Some(entry));
    }
}

pub(crate) fn ancestry(index: &Index, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    iter::successors(Some(node), |current| index.parent(*current))
}
