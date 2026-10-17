use std::collections::HashSet;

use serde::Deserialize;

use crate::eval_task::EvalTask;

const BUNDLED_TASKS: &str = include_str!("../tasks.json");

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSet {
    pub tasks: Vec<EvalTask>,
}

impl TaskSet {
    pub fn bundled() -> Self {
        let set: Self = serde_json::from_str(BUNDLED_TASKS).expect("the bundled tasks are valid JSON");
        let mut identifiers = HashSet::new();
        for task in &set.tasks {
            assert!(identifiers.insert(task.id.as_str()), "task {} repeats", task.id);
            assert!(
                task.expected_tool.is_none() || task.needs_tool,
                "task {} expects a tool without needing one",
                task.id
            );
        }
        set
    }
}
