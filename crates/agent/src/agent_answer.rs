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
        Self {
            text: answer.answer,
            hypotheses,
            actions: suggested_actions(context, &steps),
            steps,
        }
    }
}

fn suggested_actions(context: &AgentContext<'_>, steps: &[ToolStep]) -> Vec<SuggestedAction> {
    let index = context.index;
    let mut seen: Vec<NodeId> = Vec::new();
    for node in steps.iter().flat_map(|step| step.nodes.iter().copied()) {
        if !seen.contains(&node) {
            seen.push(node);
        }
    }
    let mut actions: Vec<SuggestedAction> = seen
        .iter()
        .filter(|node| !has_ancestor_in(index, **node, &seen))
        .filter(|node| context.denylist.verdict(index, **node).is_none())
        .filter_map(|node| {
            let rule = context.rules.rule(context.rules.claim_of(index, *node)?.rule);
            let actionable = rule.level != SafetyLevel::CheckYourself && !rule.is_info_only();
            actionable.then(|| SuggestedAction {
                node: *node,
                path: index.path(*node),
                rule_id: rule.id,
                level: rule.level,
                allocated: index.node(*node).allocated,
            })
        })
        .collect();
    actions.sort_by_key(|action| std::cmp::Reverse(action.allocated));
    actions
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
