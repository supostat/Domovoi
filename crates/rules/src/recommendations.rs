use std::collections::BTreeSet;
use std::path::PathBuf;

use domovoi_index::{Index, NodeId, Reclaimable, SafetyLevel};

use crate::deny_reason::DenyReason;
use crate::denylist::Denylist;
use crate::rule_set::RuleSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCard {
    pub rule: usize,
    pub items: Vec<NodeId>,
    pub blocked: Vec<(NodeId, DenyReason)>,
    pub reclaimable: Reclaimable,
    pub owner_running: bool,
    pub preselected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hypothesis {
    pub path: PathBuf,
    pub explanation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HypothesisCard {
    pub hypothesis: Hypothesis,
    pub node: NodeId,
    pub reclaimable: Reclaimable,
}

impl HypothesisCard {
    pub fn level(&self) -> SafetyLevel {
        SafetyLevel::CheckYourself
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelTotals {
    pub safe: u64,
    pub recoverable: u64,
    pub check_yourself: u64,
}

impl LevelTotals {
    fn add(&mut self, level: SafetyLevel, bytes: u64) {
        match level {
            SafetyLevel::Safe => self.safe += bytes,
            SafetyLevel::Recoverable => self.recoverable += bytes,
            SafetyLevel::CheckYourself => self.check_yourself += bytes,
        }
    }

    pub fn total(&self) -> u64 {
        self.safe + self.recoverable + self.check_yourself
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recommendations {
    pub cards: Vec<RuleCard>,
    pub totals: LevelTotals,
    pub hypotheses: Vec<HypothesisCard>,
}

impl Recommendations {
    pub fn build(
        index: &Index,
        rules: &RuleSet,
        denylist: &Denylist,
        running_bundle_ids: &BTreeSet<String>,
        hypotheses: Vec<Hypothesis>,
    ) -> Self {
        let mut cards: Vec<RuleCard> = Vec::new();
        for claim in rules.claims(index) {
            let position = if let Some(position) = cards.iter().position(|card| card.rule == claim.rule) {
                position
            } else {
                let rule = rules.rule(claim.rule);
                cards.push(RuleCard {
                    rule: claim.rule,
                    items: Vec::new(),
                    blocked: Vec::new(),
                    reclaimable: Reclaimable::default(),
                    owner_running: rule
                        .owner_bundle
                        .is_some_and(|owner| running_bundle_ids.contains(owner)),
                    preselected: rule.level == SafetyLevel::Safe && !rule.is_info_only(),
                });
                cards.len() - 1
            };
            let card = &mut cards[position];
            if let Some(reason) = denylist.verdict(index, claim.node) {
                card.blocked.push((claim.node, reason));
            } else {
                card.reclaimable += index.reclaimable_estimate(claim.node);
                card.items.push(claim.node);
            }
        }
        let mut totals = LevelTotals::default();
        for card in &cards {
            let rule = rules.rule(card.rule);
            if !rule.is_info_only() {
                totals.add(rule.level, card.reclaimable.bytes);
            }
        }
        cards.retain(|card| !card.items.is_empty() || !card.blocked.is_empty());
        cards.sort_by_key(|card| std::cmp::Reverse(card.reclaimable.bytes));
        let hypotheses = hypotheses
            .into_iter()
            .filter_map(|hypothesis| {
                let node = index.find(&hypothesis.path)?;
                Some((hypothesis, node))
            })
            .filter(|(_, node)| denylist.verdict(index, *node).is_none())
            .map(|(hypothesis, node)| HypothesisCard {
                reclaimable: index.reclaimable_estimate(node),
                hypothesis,
                node,
            })
            .collect();
        Self {
            cards,
            totals,
            hypotheses,
        }
    }
}
