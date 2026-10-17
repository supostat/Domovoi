use domovoi_rules::Hypothesis;

use crate::suggested_action::SuggestedAction;
use crate::tool_step::ToolStep;

#[derive(Clone, Debug, PartialEq)]
pub struct AgentAnswer {
    pub text: String,
    pub hypotheses: Vec<Hypothesis>,
    pub actions: Vec<SuggestedAction>,
    pub steps: Vec<ToolStep>,
}
