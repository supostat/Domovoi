#![forbid(unsafe_code)]

#[cfg(feature = "claude-code")]
mod claude_evaluation;
mod earlier_turn;
mod eval_metrics;
mod eval_task;
mod evaluation;
mod fixture_index;
mod judge;
mod task_outcome;
mod task_set;
mod tpot_comparison;

#[cfg(feature = "claude-code")]
pub use claude_evaluation::ClaudeEvaluation;
pub use earlier_turn::EarlierTurn;
pub use eval_metrics::EvalMetrics;
pub use eval_task::EvalTask;
pub use evaluation::Evaluation;
pub use fixture_index::{FIXTURE_HOME, FixtureIndex};
pub use judge::Judge;
pub use task_outcome::TaskOutcome;
pub use task_set::TaskSet;
pub use tpot_comparison::TpotComparison;
