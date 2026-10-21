use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeCodeChoice {
    pub found: bool,
    pub chosen: bool,
}
