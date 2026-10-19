use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::mem;

use domovoi_index::{DeltaApplied, Index, NodeId, SplicedDirectory};

use crate::claim_ledger::{ClaimLedger, ancestry};
use crate::claim_scope::ClaimScope;
use crate::denylist::Denylist;
use crate::recommendations::{HypothesisCard, Recommendations};
use crate::rule_set::RuleSet;

impl Recommendations {
    pub fn apply_delta(
        &mut self,
        index: &Index,
        rules: &RuleSet,
        denylist: &Denylist,
        running_bundle_ids: &BTreeSet<String>,
        applied: &DeltaApplied,
    ) {
        let hypotheses = mem::take(&mut self.hypotheses);
        let DeltaApplied::Spliced { directories, regrouped } = applied else {
            let hypotheses = hypotheses.into_iter().map(|card| card.hypothesis).collect();
            *self = Self::build(index, rules, denylist, running_bundle_ids, hypotheses);
            return;
        };
        let relocated: HashMap<NodeId, NodeId> = directories
            .iter()
            .flat_map(|spliced| spliced.kept.iter().map(|kept| (kept.previous, kept.current)))
            .collect();
        let mut ledger = ClaimLedger::of_cards(mem::take(&mut self.cards));
        ledger.relocate(&relocated);
        ledger.forget_detached(index);
        reclaim_spliced(&mut ledger, index, rules, directories);
        for file in regrouped {
            if let Some(holder) = ledger.holder_of(index, *file) {
                ledger.reassess(holder);
            }
        }
        ledger.settle(index, denylist);
        (self.cards, self.totals) = ledger.into_cards(rules, running_bundle_ids);
        if !hypotheses.is_empty() {
            let touched: HashSet<NodeId> = directories
                .iter()
                .map(|spliced| spliced.directory)
                .chain(regrouped.iter().copied())
                .flat_map(|node| ancestry(index, node))
                .collect();
            self.hypotheses = hypotheses
                .into_iter()
                .filter_map(|card| card.after_delta(index, denylist, &relocated, &touched))
                .collect();
        }
    }
}

impl HypothesisCard {
    fn after_delta(
        self,
        index: &Index,
        denylist: &Denylist,
        relocated: &HashMap<NodeId, NodeId>,
        touched: &HashSet<NodeId>,
    ) -> Option<Self> {
        let node = index.find(&self.hypothesis.path)?;
        let untouched = !touched.contains(&node);
        if node == self.node && untouched {
            return Some(self);
        }
        if denylist.verdict(index, node).is_some() {
            return None;
        }
        let subtree_kept = untouched && (node == self.node || relocated.get(&self.node) == Some(&node));
        let reclaimable = if subtree_kept {
            self.reclaimable
        } else {
            index.reclaimable_estimate(node)
        };
        Some(Self {
            node,
            reclaimable,
            ..self
        })
    }
}

fn reclaim_spliced(ledger: &mut ClaimLedger, index: &Index, rules: &RuleSet, directories: &[SplicedDirectory]) {
    let mut shallow_first: Vec<&SplicedDirectory> = directories.iter().collect();
    shallow_first.sort_by_cached_key(|spliced| index.depth_of(spliced.directory));
    let mut rewalked: HashSet<NodeId> = HashSet::new();
    for spliced in shallow_first {
        if ancestry(index, spliced.directory).any(|node| rewalked.contains(&node)) {
            continue;
        }
        match rules.scope_of(index, spliced.directory) {
            ClaimScope::Inside(claim) => {
                ledger.forget_within(index, claim.node);
                ledger.claim(claim);
            }
            ClaimScope::Open { components, effective } => {
                ledger.forget(spliced.directory);
                let kept: HashSet<NodeId> = spliced.kept.iter().map(|kept| kept.current).collect();
                let changed_siblings: Vec<&OsStr> = spliced
                    .appeared
                    .iter()
                    .chain(&spliced.vanished)
                    .map(OsString::as_os_str)
                    .collect();
                for child in index.children(spliced.directory) {
                    if kept.contains(&child) {
                        if !rules.follows_siblings(index, child, &changed_siblings) {
                            continue;
                        }
                        ledger.forget_within(index, child);
                        rewalked.insert(child);
                    }
                    for claim in rules.claims_from(index, &components, child, effective) {
                        ledger.claim(claim);
                    }
                }
            }
        }
    }
}
