use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

use domovoi_index::{Index, LastOpenedSource, NodeId, Reclaimable, SafetyLevel};
use domovoi_rules::{Condition, DenyReason, HypothesisCard, ProjectCard, Recommendations, RuleCard, RuleSet};
use domovoi_theme::{DateSource, format_age, format_count, format_elapsed, format_size};
use serde::Deserialize;

use crate::application_name::running_application_name;
use crate::card_model::CardModel;
use crate::deny_reason_text::deny_reason_text;
use crate::owner_reason::evidence_phrase;
use crate::project_row::ProjectRow;
use crate::provenance::Provenance;
use crate::safety_badge::SafetyBadge;
use crate::strings::{
    CAN_FREE, NOT_TOUCHED, PATHS, PROJECTS_EXPLANATION, PROJECTS_TITLE, SPLIT_CHECK_YOURSELF, SPLIT_RECOVERABLE,
    SPLIT_SAFE, VENDOR_COMMAND, rule_title,
};
use crate::tilde::tilde;
use crate::tray_target::TrayTarget;

const STALE_AFTER: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const REPEATED: &str = "×";

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
        let mut cards: Vec<CardModel> = recommendations
            .cards
            .iter()
            .map(|card| rule_card(index, rules, card))
            .collect();
        if !recommendations.projects.is_empty() {
            let projects = projects_card(index, rules, &recommendations.projects, now);
            let position = cards
                .iter()
                .position(|card| card.amount < projects.amount)
                .unwrap_or(cards.len());
            cards.insert(position, projects);
        }
        cards.extend(
            recommendations
                .hypotheses
                .iter()
                .map(|hypothesis| hypothesis_card(index, hypothesis, now)),
        );
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

    pub fn keep_selection(&mut self, previous: &Self) {
        for card in self.cards.iter_mut().filter(|card| card.selectable()) {
            let Some(old) = previous.cards.iter().find(|old| old.is_same_card_as(card)) else {
                continue;
            };
            if card.rows.is_empty() {
                card.selected = old.selected;
            } else {
                for row in &mut card.rows {
                    if let Some(old_row) = old.rows.iter().find(|old_row| old_row.path == row.path) {
                        row.selected = old_row.selected && row.amount > 0;
                    }
                }
                card.selected = card.every_row_selected();
            }
        }
    }

    pub fn selected_total(&self) -> u64 {
        self.cards.iter().map(CardModel::selected_amount).sum()
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
        paths: card.items.iter().map(|item| tilde(&index.path(item.node))).collect(),
        path_reasons: if rule.condition == Some(Condition::OwnerMissing) {
            card.items
                .iter()
                .map(|item| {
                    index
                        .ownership(item.node)
                        .map(|ownership| evidence_phrase(ownership.evidence).to_owned())
                })
                .collect()
        } else {
            Vec::new()
        },
        explanation: rule.explanation.to_owned(),
        level: SafetyBadge::from(rule.level),
        provenance: Provenance::Rule(rule.id.to_owned()),
        amount: card.reclaimable.bytes,
        held_by_snapshots: card.reclaimable.held_by_snapshots,
        held_by_clones: card.reclaimable.held_by_clones,
        held_by_hard_links: card.reclaimable.held_by_hard_links,
        owner_running,
        blocked: blocked_line(&card.blocked.iter().map(|(_, reason)| reason).collect::<Vec<_>>()),
        vendor_command: rule.vendor_command.map(|command| format!("{VENDOR_COMMAND} {command}")),
        age: None,
        selected: card.preselected,
        targets: card
            .items
            .iter()
            .map(|item| TrayTarget {
                node: item.node,
                rule_id: Some(rule.id.to_owned()),
                project: None,
            })
            .collect(),
        rows: Vec::new(),
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
        path_reasons: Vec::new(),
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
        rows: Vec::new(),
        targets: vec![TrayTarget {
            node: card.node,
            rule_id: None,
            project: None,
        }],
    }
}

fn projects_card(index: &Index, rules: &RuleSet, projects: &[ProjectCard], now: SystemTime) -> CardModel {
    let (stale, fresh): (Vec<ProjectRow>, Vec<ProjectRow>) = projects
        .iter()
        .map(|project| project_row(index, rules, project, now))
        .partition(|row| row.stale);
    let rows: Vec<ProjectRow> = stale.into_iter().chain(fresh).collect();
    let mut items: Vec<(u64, String)> = projects
        .iter()
        .flat_map(|project| &project.items)
        .map(|item| (item.reclaimable.bytes, tilde(&index.path(item.node))))
        .collect();
    items.sort_by_key(|(bytes, _)| std::cmp::Reverse(*bytes));
    let mut held = Reclaimable::default();
    for project in projects {
        held += project.reclaimable;
    }
    let blocked: Vec<&DenyReason> = projects
        .iter()
        .flat_map(|project| &project.blocked)
        .map(|blocked| &blocked.reason)
        .collect();
    let mut card = CardModel {
        title: PROJECTS_TITLE.to_owned(),
        paths: items.into_iter().map(|(_, path)| path).collect(),
        path_reasons: Vec::new(),
        explanation: PROJECTS_EXPLANATION.to_owned(),
        level: SafetyBadge::from(SafetyLevel::Safe),
        provenance: Provenance::Projects,
        amount: held.bytes,
        held_by_snapshots: held.held_by_snapshots,
        held_by_clones: held.held_by_clones,
        held_by_hard_links: held.held_by_hard_links,
        owner_running: None,
        blocked: blocked_line(&blocked),
        vendor_command: None,
        age: None,
        selected: false,
        targets: rows.iter().flat_map(|row| row.targets.iter().cloned()).collect(),
        rows,
    };
    card.selected = card.every_row_selected();
    card
}

fn project_row(index: &Index, rules: &RuleSet, project: &ProjectCard, now: SystemTime) -> ProjectRow {
    let root = index.path(project.root);
    let last_work = project.last_work.system_time();
    let stale = last_work.is_none_or(|then| now.duration_since(then).unwrap_or_default() > STALE_AFTER);
    ProjectRow {
        name: root
            .file_name()
            .map_or_else(|| tilde(&root), |name| name.to_string_lossy().into_owned()),
        path: tilde(&root),
        artifacts: artifact_names(index, project),
        age: last_work.map_or_else(String::new, |then| format_elapsed(then, now)),
        stale,
        amount: project.reclaimable.bytes,
        selected: stale && project.reclaimable.bytes > 0,
        targets: project
            .items
            .iter()
            .map(|item| TrayTarget {
                node: item.node,
                rule_id: Some(rules.rule(item.rule).id.to_owned()),
                project: Some(root.clone()),
            })
            .collect(),
    }
}

fn artifact_names(index: &Index, project: &ProjectCard) -> Vec<String> {
    let mut counted: Vec<(String, usize)> = Vec::new();
    for item in &project.items {
        let name = index.name(item.node).to_string_lossy().into_owned();
        match counted.iter_mut().find(|(known, _)| *known == name) {
            Some((_, count)) => *count += 1,
            None => counted.push((name, 1)),
        }
    }
    counted
        .into_iter()
        .map(|(name, count)| {
            if count == 1 {
                name
            } else {
                format!("{name} {REPEATED}{count}")
            }
        })
        .collect()
}

fn blocked_line(reasons: &[&DenyReason]) -> Option<String> {
    if reasons.is_empty() {
        return None;
    }
    let texts: BTreeSet<&str> = reasons.iter().map(|reason| deny_reason_text(reason)).collect();
    Some(format!(
        "{} {NOT_TOUCHED}: {}",
        format_count(reasons.len() as u64, &PATHS),
        texts.into_iter().collect::<Vec<_>>().join(", ")
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
