use crate::purge_skip::PurgeSkip;
use crate::trash_journal::JournalEntry;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub purged: Vec<JournalEntry>,
    pub interrupted: Vec<JournalEntry>,
    pub untouched: Vec<JournalEntry>,
    pub skipped: Vec<(JournalEntry, PurgeSkip)>,
    pub failed: Vec<(JournalEntry, String)>,
}
