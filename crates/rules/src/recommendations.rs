use std::collections::BTreeSet;
use std::path::PathBuf;

use domovoi_index::{Index, NodeId, Reclaimable, SafetyLevel};

use crate::card_item::CardItem;
use crate::claim_ledger::ClaimLedger;
use crate::deny_reason::DenyReason;
use crate::denylist::Denylist;
use crate::rule_set::RuleSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCard {
    pub rule: usize,
    pub items: Vec<CardItem>,
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

    pub(crate) fn assess(index: &Index, denylist: &Denylist, hypothesis: Hypothesis) -> Option<Self> {
        let node = index.find(&hypothesis.path)?;
        if denylist.verdict(index, node).is_some() {
            return None;
        }
        Some(Self {
            reclaimable: index.reclaimable_estimate(node),
            hypothesis,
            node,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelTotals {
    pub safe: u64,
    pub recoverable: u64,
    pub check_yourself: u64,
}

impl LevelTotals {
    pub(crate) fn add(&mut self, level: SafetyLevel, bytes: u64) {
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
        let mut ledger = ClaimLedger::of_claims(rules.claims(index));
        ledger.settle(index, denylist);
        let (cards, totals) = ledger.into_cards(rules, running_bundle_ids);
        Self {
            cards,
            totals,
            hypotheses: hypotheses
                .into_iter()
                .filter_map(|hypothesis| HypothesisCard::assess(index, denylist, hypothesis))
                .collect(),
        }
    }
}
