use std::collections::BTreeSet;
use std::time::SystemTime;

use domovoi_index::{Index, LastOpenedSource, NodeId};
use domovoi_rules::{HypothesisCard, Recommendations, RuleCard, RuleSet};
use domovoi_theme::{DateSource, format_age, format_count, format_size};
use serde::Deserialize;

use crate::application_name::running_application_name;
use crate::card_model::CardModel;
use crate::deny_reason_text::deny_reason_text;
use crate::provenance::Provenance;
use crate::safety_badge::SafetyBadge;
use crate::strings::{
    CAN_FREE, NOT_TOUCHED, PATHS, SPLIT_CHECK_YOURSELF, SPLIT_RECOVERABLE, SPLIT_SAFE, VENDOR_COMMAND, rule_title,
};
use crate::tilde::tilde;
use crate::tray_target::TrayTarget;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecommendationsModel {
    pub safe: u64,
    pub recoverable: u64,
    pub check_yourself: u64,
    pub cards: Vec<CardModel>,
}

impl RecommendationsModel {
    pub fn build(index: &Index, rules: &RuleSet, recommendations: &Recommendations, now: SystemTime) -> Self {
        let cards = recommendations
            .cards
            .iter()
            .map(|card| rule_card(index, rules, card))
            .chain(
                recommendations
                    .hypotheses
                    .iter()
                    .map(|hypothesis| hypothesis_card(index, hypothesis, now)),
            )
            .collect();
        Self {
            safe: recommendations.totals.safe,
            recoverable: recommendations.totals.recoverable,
            check_yourself: recommendations.totals.check_yourself,
            cards,
        }
    }

    pub fn total(&self) -> u64 {
        self.safe + self.recoverable + self.check_yourself
    }

    pub fn total_line(&self) -> String {
        format!("{CAN_FREE} {}", format_size(self.total()))
    }

    pub fn split_line(&self) -> String {
        format!(
            "{SPLIT_SAFE} {} · {SPLIT_RECOVERABLE} {} · {SPLIT_CHECK_YOURSELF} {}",
            format_size(self.safe),
            format_size(self.recoverable),
            format_size(self.check_yourself)
        )
    }

    pub fn selected_total(&self) -> u64 {
        self.cards
            .iter()
            .filter(|card| card.selected && card.selectable())
            .map(|card| card.amount)
            .sum()
    }
}

fn rule_card(index: &Index, rules: &RuleSet, card: &RuleCard) -> CardModel {
    let rule = rules.rule(card.rule);
    let owner_running = card
        .owner_running
        .then_some(rule.owner_bundle)
        .flatten()
        .map(running_application_name);
    let model = CardModel {
        title: rule_title(rule.id)
            .expect("every rule of the table has a title")
            .to_owned(),
        paths: card.items.iter().map(|node| tilde(&index.path(*node))).collect(),
        explanation: rule.explanation.to_owned(),
        level: SafetyBadge::from(rule.level),
        provenance: Provenance::Rule(rule.id.to_owned()),
        amount: card.reclaimable.bytes,
        held_by_snapshots: card.reclaimable.held_by_snapshots,
        held_by_clones: card.reclaimable.held_by_clones,
        held_by_hard_links: card.reclaimable.held_by_hard_links,
        owner_running,
        blocked: blocked_line(card),
        vendor_command: rule.vendor_command.map(|command| format!("{VENDOR_COMMAND} {command}")),
        age: None,
        selected: card.preselected,
        targets: card
            .items
            .iter()
            .map(|node| TrayTarget {
                node: *node,
                rule_id: Some(rule.id.to_owned()),
            })
            .collect(),
    };
    CardModel {
        selected: model.selected && model.selectable(),
        ..model
    }
}

fn hypothesis_card(index: &Index, card: &HypothesisCard, now: SystemTime) -> CardModel {
    CardModel {
        title: card.hypothesis.path.file_name().map_or_else(
            || tilde(&card.hypothesis.path),
            |name| name.to_string_lossy().into_owned(),
        ),
        paths: vec![tilde(&card.hypothesis.path)],
        explanation: card.hypothesis.explanation.clone(),
        level: SafetyBadge::from(card.level()),
        provenance: Provenance::Hypothesis,
        amount: card.reclaimable.bytes,
        held_by_snapshots: card.reclaimable.held_by_snapshots,
        held_by_clones: card.reclaimable.held_by_clones,
        held_by_hard_links: card.reclaimable.held_by_hard_links,
        owner_running: None,
        blocked: None,
        vendor_command: None,
        age: last_opened(index, card.node, now),
        selected: false,
        targets: vec![TrayTarget {
            node: card.node,
            rule_id: None,
        }],
    }
}

fn blocked_line(card: &RuleCard) -> Option<String> {
    if card.blocked.is_empty() {
        return None;
    }
    let reasons: BTreeSet<&str> = card
        .blocked
        .iter()
        .map(|(_, reason)| deny_reason_text(reason))
        .collect();
    Some(format!(
        "{} {NOT_TOUCHED}: {}",
        format_count(card.blocked.len() as u64, &PATHS),
        reasons.into_iter().collect::<Vec<_>>().join(", ")
    ))
}

fn last_opened(index: &Index, node: NodeId, now: SystemTime) -> Option<String> {
    let record = index.node(node);
    let source = match record.last_opened_source {
        LastOpenedSource::Xattr => DateSource::LastOpened,
        LastOpenedSource::SpotlightIndex => DateSource::LastOpenedBySpotlight,
        LastOpenedSource::Inherited => DateSource::LastOpenedInherited,
        LastOpenedSource::Unknown => return None,
    };
    record
        .last_opened
        .system_time()
        .map(|then| format_age(then, now, source))
}
