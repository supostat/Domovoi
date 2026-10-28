use domovoi_grammar::FinalAnswer;
use domovoi_index::{Index, NodeId, SafetyLevel};
use domovoi_rules::Hypothesis;

use crate::agent_context::AgentContext;
use crate::home_path::resolved;
use crate::suggested_action::SuggestedAction;
use crate::tool_step::ToolStep;

#[derive(Clone, Debug, PartialEq)]
pub struct AgentAnswer {
    pub text: String,
    pub hypotheses: Vec<Hypothesis>,
    pub actions: Vec<SuggestedAction>,
    pub steps: Vec<ToolStep>,
}

impl AgentAnswer {
    pub(crate) fn assembled(context: &AgentContext<'_>, answer: FinalAnswer, steps: Vec<ToolStep>) -> Self {
        let hypotheses = answer
            .hypotheses
            .into_iter()
            .map(|hypothesis| {
                (
                    resolved(&hypothesis.path, context.home, context.index),
                    hypothesis.guess,
                )
            })
            .filter(|(path, _)| context.index.find(path).is_some())
            .map(|(path, explanation)| Hypothesis { path, explanation })
            .collect();
        let candidates: Vec<NodeId> = if answer.targets.is_empty() {
            steps.iter().flat_map(|step| step.nodes.iter().copied()).collect()
        } else {
            answer
                .targets
                .iter()
                .filter_map(|target| context.index.find(&resolved(target, context.home, context.index)))
                .collect()
        };
        Self {
            text: answer.answer,
            hypotheses,
            actions: suggested_actions(context, candidates),
            steps,
        }
    }
}

fn suggested_actions(context: &AgentContext<'_>, candidates: Vec<NodeId>) -> Vec<SuggestedAction> {
    let mut seen: Vec<NodeId> = Vec::new();
    for node in candidates {
        if !seen.contains(&node) {
            seen.push(node);
        }
    }
    let mut actions: Vec<SuggestedAction> = seen
        .iter()
        .filter(|node| !has_ancestor_in(context.index, **node, &seen))
        .filter_map(|node| suggested_action(context, *node))
        .collect();
    actions.sort_by_key(|action| std::cmp::Reverse(action.allocated));
    actions
}

pub(crate) fn suggested_action(context: &AgentContext<'_>, node: NodeId) -> Option<SuggestedAction> {
    let index = context.index;
    if context.denylist.verdict(index, node).is_some() {
        return None;
    }
    let rule = context.rules.rule(context.rules.claim_of(index, node)?.rule);
    let actionable = rule.level != SafetyLevel::CheckYourself && !rule.is_info_only();
    actionable.then(|| SuggestedAction {
        node,
        path: index.path(node),
        rule_id: rule.id,
        level: rule.level,
        allocated: index.node(node).allocated,
    })
}

fn has_ancestor_in(index: &Index, node: NodeId, candidates: &[NodeId]) -> bool {
    let mut current = node;
    while let Some(parent) = index.parent(current) {
        if candidates.contains(&parent) {
            return true;
        }
        current = parent;
    }
    false
}
