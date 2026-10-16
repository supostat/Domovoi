use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Component, Path};

use domovoi_apfs_sys::volume_capabilities;
use domovoi_index::{Index, NodeId, SafetyLevel, SafetyLevels};

use crate::config_overrides::ConfigOverrides;
use crate::name_match::{CaseRule, normalized};
use crate::rule::{Condition, Rule};
use crate::rule_table::rule_table;
use crate::simulator_runtimes::{InstalledSimulatorRuntimes, SimulatorRuntimes};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    pub node: NodeId,
    pub rule: usize,
}

pub struct RuleSet {
    rules: Vec<Rule>,
    case: CaseRule,
    runtimes: Box<dyn SimulatorRuntimes>,
    anchored_by_length: HashMap<usize, Vec<(usize, usize)>>,
    any_depth: Vec<(usize, usize)>,
}

impl fmt::Debug for RuleSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuleSet")
            .field("rules", &self.rules.len())
            .field("case", &self.case)
            .finish_non_exhaustive()
    }
}

impl RuleSet {
    pub fn new(rules: Vec<Rule>, case: CaseRule, runtimes: Box<dyn SimulatorRuntimes>) -> Self {
        let rules: Vec<Rule> = rules
            .into_iter()
            .map(|mut rule| {
                rule.patterns = rule.patterns.iter().map(|pattern| pattern.folded(case)).collect();
                rule
            })
            .collect();
        let mut anchored_by_length: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
        let mut any_depth = Vec::new();
        for (rule_index, rule) in rules.iter().enumerate() {
            for (pattern_index, pattern) in rule.patterns.iter().enumerate() {
                match pattern.anchored_length() {
                    Some(length) => anchored_by_length
                        .entry(length)
                        .or_default()
                        .push((rule_index, pattern_index)),
                    None => any_depth.push((rule_index, pattern_index)),
                }
            }
        }
        Self {
            rules,
            case,
            runtimes,
            anchored_by_length,
            any_depth,
        }
    }

    pub fn for_volume(volume_root: &Path, overrides: &ConfigOverrides) -> io::Result<Self> {
        let case = if volume_capabilities(volume_root)?.case_sensitive {
            CaseRule::Sensitive
        } else {
            CaseRule::Insensitive
        };
        Ok(Self::new(
            rule_table(overrides),
            case,
            Box::new(InstalledSimulatorRuntimes::discover()),
        ))
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn rule(&self, index: usize) -> &Rule {
        &self.rules[index]
    }

    pub fn claims(&self, index: &Index) -> Vec<Claim> {
        let mut claims = Vec::new();
        let mut components = self.root_components(index);
        let root_depth = components.len();
        let mut pending: Vec<(NodeId, usize, Option<usize>)> = index
            .children(index.root())
            .map(|child| (child, root_depth + 1, None))
            .collect();
        while let Some((node, depth, inherited)) = pending.pop() {
            components.truncate(depth - 1);
            components.push(normalized(index.name(node), self.case));
            let effective = self.matching_rule(index, node, &components).or(inherited);
            let has_children = index.children(node).next().is_some();
            match effective {
                Some(rule) if !has_children || !self.deeper_rule_possible(&components) => {
                    claims.push(Claim { node, rule });
                }
                _ => pending.extend(index.children(node).map(|child| (child, depth + 1, effective))),
            }
        }
        claims
    }

    fn deeper_rule_possible(&self, components: &[String]) -> bool {
        self.anchored_by_length
            .iter()
            .filter(|(length, _)| **length > components.len())
            .flat_map(|(_, patterns)| patterns)
            .any(|(rule, pattern)| self.rules[*rule].patterns[*pattern].prefix_matches(components))
    }

    pub fn claim_of(&self, index: &Index, node: NodeId) -> Option<Claim> {
        let mut chain = vec![node];
        let mut current = node;
        while let Some(parent) = index.parent(current) {
            chain.push(parent);
            current = parent;
        }
        chain.pop();
        let mut components = self.root_components(index);
        for member in chain.into_iter().rev() {
            components.push(normalized(index.name(member), self.case));
            if let Some(rule) = self.matching_rule(index, member, &components) {
                return Some(Claim { node: member, rule });
            }
        }
        None
    }

    fn root_components(&self, index: &Index) -> Vec<String> {
        index
            .path(index.root())
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(normalized(name, self.case)),
                _ => None,
            })
            .collect()
    }

    fn matching_rule(&self, index: &Index, node: NodeId, components: &[String]) -> Option<usize> {
        let has_sibling = |marker: &str| {
            index
                .parent(node)
                .and_then(|parent| index.child_named(parent, OsStr::new(marker)))
                .is_some()
        };
        let anchored = self.anchored_by_length.get(&components.len()).into_iter().flatten();
        let mut matching: Vec<usize> = anchored
            .chain(self.any_depth.iter())
            .filter(|(rule, pattern)| self.rules[*rule].patterns[*pattern].matches(components, has_sibling))
            .map(|(rule, _)| *rule)
            .collect();
        matching.sort_unstable();
        matching.dedup();
        matching
            .into_iter()
            .find(|rule| self.condition_holds(index, node, *rule))
    }

    fn condition_holds(&self, index: &Index, node: NodeId, rule: usize) -> bool {
        match self.rules[rule].condition {
            None => true,
            Some(Condition::SimulatorRuntimeMissing) => {
                self.runtimes.device_runtime_missing(&index.physical_path(node))
            }
        }
    }
}

impl SafetyLevels for RuleSet {
    fn level(&self, index: &Index, node: NodeId) -> Option<SafetyLevel> {
        self.claim_of(index, node).map(|claim| self.rules[claim.rule].level)
    }
}
