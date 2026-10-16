#![forbid(unsafe_code)]

mod config_overrides;
mod deny_reason;
mod denylist;
mod name_match;
mod path_pattern;
mod recommendations;
mod rule;
mod rule_set;
mod rule_table;
mod running_applications;
mod simulator_runtimes;
mod trash_failure;
mod trash_journal;
mod tray;

pub use config_overrides::ConfigOverrides;
pub use deny_reason::DenyReason;
pub use denylist::Denylist;
pub use name_match::CaseRule;
pub use recommendations::{Hypothesis, HypothesisCard, LevelTotals, Recommendations, RuleCard};
pub use rule::{Condition, Rule};
pub use rule_set::{Claim, RuleSet};
pub use rule_table::rule_table;
pub use running_applications::running_bundle_ids;
pub use simulator_runtimes::{InstalledSimulatorRuntimes, SimulatorRuntimes};
pub use trash_failure::TrashFailure;
pub use trash_journal::{JournalEntry, RestoreError, TrashJournal};
pub use tray::{TrashOutcome, Tray, TrayItem, open_trash};
