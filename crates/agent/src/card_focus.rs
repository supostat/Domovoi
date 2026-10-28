use domovoi_index::SafetyLevel;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardFocus {
    pub rule: Option<String>,
    pub title: String,
    pub level: SafetyLevel,
    pub paths: Vec<String>,
    pub bytes: u64,
}
