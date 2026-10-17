use domovoi_index::SafetyLevel;

pub(crate) fn level_name(level: SafetyLevel) -> &'static str {
    match level {
        SafetyLevel::Safe => "safe",
        SafetyLevel::Recoverable => "recoverable",
        SafetyLevel::CheckYourself => "check_yourself",
    }
}

pub(crate) fn level_named(name: &str) -> Option<SafetyLevel> {
    [SafetyLevel::Safe, SafetyLevel::Recoverable, SafetyLevel::CheckYourself]
        .into_iter()
        .find(|level| level_name(*level) == name)
}
