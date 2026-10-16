use domovoi_index::SafetyLevel;

use crate::path_pattern::PathPattern;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    SimulatorRuntimeMissing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub level: SafetyLevel,
    pub explanation: &'static str,
    pub owner_bundle: Option<&'static str>,
    pub vendor_command: Option<&'static str>,
    pub condition: Option<Condition>,
    pub(crate) patterns: Vec<PathPattern>,
}

impl Rule {
    pub(crate) fn new(id: &'static str, level: SafetyLevel, patterns: &[&str], explanation: &'static str) -> Self {
        Self {
            id,
            level,
            explanation,
            owner_bundle: None,
            vendor_command: None,
            condition: None,
            patterns: patterns.iter().map(|pattern| PathPattern::parse(pattern)).collect(),
        }
    }

    pub(crate) fn owned_by(mut self, bundle_id: &'static str) -> Self {
        self.owner_bundle = Some(bundle_id);
        self
    }

    pub(crate) fn info(mut self, vendor_command: &'static str) -> Self {
        self.vendor_command = Some(vendor_command);
        self
    }

    pub(crate) fn when(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub(crate) fn also_at(mut self, pattern: PathPattern) -> Self {
        self.patterns.push(pattern);
        self
    }

    pub fn is_info_only(&self) -> bool {
        self.vendor_command.is_some()
    }
}
