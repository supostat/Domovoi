use std::cell::OnceCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::Path;

use domovoi_apfs_sys::volume_capabilities;
use domovoi_index::{App, Badges, Index, NodeId, NodeKind, SafetyLevel, SafetyLevels};

use crate::claim_frame::ClaimFrame;
use crate::claim_ledger::ancestry;
use crate::claim_scope::ClaimScope;
use crate::config_overrides::ConfigOverrides;
use crate::name_match::{CaseRule, normalized};
use crate::path_pattern::PathPattern;
use crate::rule::{Condition, Rule};
use crate::rule_table::rule_table;
use crate::simulator_runtimes::{InstalledSimulatorRuntimes, SimulatorRuntimes};
use crate::tool_roots::TOOL_ROOTS;

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
    tool_roots: Vec<PathPattern>,
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
            tool_roots: TOOL_ROOTS
                .iter()
                .map(|root| PathPattern::parse(root).folded(case))
                .collect(),
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
        let components = self.root_components(index);
        index
            .children(index.root())
            .flat_map(|child| self.claims_from(index, &components, child, None))
            .collect()
    }

    pub(crate) fn claims_from(
        &self,
        index: &Index,
        parent_components: &[String],
        top: NodeId,
        inherited: Option<usize>,
    ) -> Vec<Claim> {
        let mut claims = Vec::new();
        let mut components = parent_components.to_vec();
        let mut pending = vec![(top, parent_components.len() + 1, inherited)];
        while let Some((node, depth, inherited)) = pending.pop() {
            components.truncate(depth - 1);
            components.push(normalized(index.name(node), self.case));
            match self.frame(index, node, &components, inherited) {
                ClaimFrame::Claimed(rule) => claims.push(Claim { node, rule }),
                ClaimFrame::Open(effective) => {
                    pending.extend(index.children(node).map(|child| (child, depth + 1, effective)));
                }
            }
        }
        claims
    }

    pub(crate) fn scope_of(&self, index: &Index, directory: NodeId) -> ClaimScope {
        let mut chain = vec![directory];
        let mut current = directory;
        while let Some(parent) = index.parent(current) {
            chain.push(parent);
            current = parent;
        }
        chain.pop();
        let mut components = self.root_components(index);
        let mut effective = None;
        for member in chain.into_iter().rev() {
            components.push(normalized(index.name(member), self.case));
            match self.frame(index, member, &components, effective) {
                ClaimFrame::Claimed(rule) => return ClaimScope::Inside(Claim { node: member, rule }),
                ClaimFrame::Open(rule) => effective = rule,
            }
        }
        ClaimScope::Open { components, effective }
    }

    pub(crate) fn follows_siblings(&self, index: &Index, node: NodeId, siblings: &[&OsStr]) -> bool {
        let name = normalized(index.name(node), self.case);
        self.any_depth.iter().any(|(rule, pattern)| {
            let pattern = &self.rules[*rule].patterns[*pattern];
            siblings.iter().any(|sibling| pattern.follows_sibling(&name, sibling))
        })
    }

    fn frame(&self, index: &Index, node: NodeId, components: &[String], inherited: Option<usize>) -> ClaimFrame {
        let effective = self.matching_rule(index, node, components).or(inherited);
        match effective {
            Some(rule)
                if index.children(node).next().is_none()
                    || !self.deeper_rule_possible(components)
                    || self.claims_owned_root_whole(rule) =>
            {
                ClaimFrame::Claimed(rule)
            }
            _ => ClaimFrame::Open(effective),
        }
    }

    fn claims_owned_root_whole(&self, rule: usize) -> bool {
        self.rules[rule].condition == Some(Condition::OwnerMissing)
    }

    fn deeper_rule_possible(&self, components: &[String]) -> bool {
        self.anchored_by_length
            .iter()
            .filter(|(length, _)| **length > components.len())
            .flat_map(|(_, patterns)| patterns)
            .any(|(rule, pattern)| self.rules[*rule].patterns[*pattern].prefix_matches(components))
    }

    pub(crate) fn project_folder(&self, index: &Index, claim: Claim) -> Option<NodeId> {
        let patterns = &self.rules[claim.rule].patterns;
        if !patterns.iter().any(PathPattern::has_project_marker) {
            return None;
        }
        ancestry(index, claim.node).find_map(|member| {
            let parent = index.parent(member)?;
            let name = normalized(index.name(member), self.case);
            let has_sibling = |marker: &str| index.child_named(parent, OsStr::new(marker)).is_some();
            patterns
                .iter()
                .any(|pattern| pattern.marks_project(&name, has_sibling))
                .then_some(parent)
        })
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
            .root_components()
            .iter()
            .map(|component| normalized(OsStr::new(component), self.case))
            .collect()
    }

    fn matching_rule(&self, index: &Index, node: NodeId, components: &[String]) -> Option<usize> {
        let has_sibling = |marker: &str| {
            index
                .parent(node)
                .and_then(|parent| index.child_named(parent, OsStr::new(marker)))
                .is_some()
        };
        let hosts_projects = OnceCell::new();
        let anchored = self.anchored_by_length.get(&components.len()).into_iter().flatten();
        let mut matching: Vec<usize> = anchored
            .chain(self.any_depth.iter())
            .filter(|(rule, pattern)| {
                let pattern = &self.rules[*rule].patterns[*pattern];
                pattern.matches(components, has_sibling)
                    && (!pattern.has_project_marker()
                        || *hosts_projects.get_or_init(|| self.hosts_projects(index, node, components)))
            })
            .map(|(rule, _)| *rule)
            .collect();
        matching.sort_unstable();
        matching.dedup();
        matching
            .into_iter()
            .find(|rule| self.condition_holds(index, node, *rule))
    }

    fn hosts_projects(&self, index: &Index, node: NodeId, components: &[String]) -> bool {
        let inside_dependencies = index
            .parent(node)
            .is_some_and(|parent| index.inside_dependency_store(parent));
        !inside_dependencies
            && !self.tool_roots.iter().any(|root| root.encloses(components))
            && index.enclosing_package(node).is_none()
    }

    fn condition_holds(&self, index: &Index, node: NodeId, rule: usize) -> bool {
        match self.rules[rule].condition {
            None => true,
            Some(Condition::SimulatorRuntimeMissing) => {
                self.runtimes.device_runtime_missing(&index.physical_path(node))
            }
            Some(Condition::OwnerMissing) => index
                .ownership(node)
                .and_then(|ownership| index.app(ownership.app))
                .is_some_and(App::is_leftover),
            Some(Condition::ScreenCapture) => {
                let record = index.node(node);
                record.kind == NodeKind::File && record.badges.contains(Badges::SCREEN_CAPTURE)
            }
        }
    }
}

impl SafetyLevels for RuleSet {
    fn level(&self, index: &Index, node: NodeId) -> Option<SafetyLevel> {
        self.claim_of(index, node).map(|claim| self.rules[claim.rule].level)
    }
}
