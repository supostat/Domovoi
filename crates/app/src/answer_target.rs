use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerTarget {
    pub path: PathBuf,
    pub rule_id: String,
}
