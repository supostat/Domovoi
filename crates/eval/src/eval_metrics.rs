use serde::Serialize;

use crate::task_outcome::TaskOutcome;
use crate::task_set::TaskSet;
use crate::tpot_comparison::TpotComparison;

const TOOL_ACCURACY: f64 = 0.9;
const SCHEMA_VALIDITY: f64 = 1.0;
const TOOL_CALL_RATE: f64 = 0.9;
const EXPLANATION_SCORE: f64 = 4.0;
const TPOT_RATIO: f64 = 2.0;
const FAILED_ANSWER_SCORE: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct EvalMetrics {
    pub tool_accuracy: f64,
    pub schema_validity: f64,
    pub tool_call_rate: f64,
    pub explanation_score: f64,
    pub tpot_ratio: f64,
}

impl EvalMetrics {
    pub fn measure(tasks: &TaskSet, outcomes: &[TaskOutcome], tpot: TpotComparison) -> Self {
        assert_eq!(tasks.tasks.len(), outcomes.len(), "every task has an outcome");
        let pairs = || tasks.tasks.iter().zip(outcomes);
        let share = |hits: usize, total: usize| if total == 0 { 1.0 } else { hits as f64 / total as f64 };
        let expected: Vec<_> = pairs()
            .filter_map(|(task, outcome)| task.expected_tool.map(|tool| outcome.first_tool_is(tool)))
            .collect();
        let needing: Vec<_> = pairs()
            .filter(|(task, _)| task.needs_tool)
            .map(|(_, outcome)| !outcome.tools.is_empty())
            .collect();
        let scores: Vec<f64> = outcomes
            .iter()
            .map(|outcome| f64::from(outcome.score.unwrap_or(FAILED_ANSWER_SCORE)))
            .collect();
        Self {
            tool_accuracy: share(expected.iter().filter(|hit| **hit).count(), expected.len()),
            schema_validity: share(
                outcomes.iter().filter(|outcome| outcome.schema_valid).count(),
                outcomes.len(),
            ),
            tool_call_rate: share(needing.iter().filter(|hit| **hit).count(), needing.len()),
            explanation_score: scores.iter().sum::<f64>() / scores.len().max(1) as f64,
            tpot_ratio: tpot.ratio(),
        }
    }

    pub fn shortfalls(&self) -> Vec<String> {
        let mut shortfalls = Vec::new();
        let mut floor = |name: &str, value: f64, threshold: f64| {
            if value < threshold {
                shortfalls.push(format!("{name} {value:.3} < {threshold}"));
            }
        };
        floor("tool accuracy", self.tool_accuracy, TOOL_ACCURACY);
        floor("schema validity", self.schema_validity, SCHEMA_VALIDITY);
        floor("tool call rate", self.tool_call_rate, TOOL_CALL_RATE);
        floor("explanation", self.explanation_score, EXPLANATION_SCORE);
        if self.tpot_ratio > TPOT_RATIO {
            shortfalls.push(format!("TPOT ratio {:.3} > {TPOT_RATIO}", self.tpot_ratio));
        }
        shortfalls
    }
}
