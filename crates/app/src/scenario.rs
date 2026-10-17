use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::scenario_step::ScenarioStep;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub root: PathBuf,
    pub state: PathBuf,
    pub steps: Vec<ScenarioStep>,
}

impl Scenario {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }
}
