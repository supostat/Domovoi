const LOCAL_MODEL: &str = "local-model";
const CLAUDE_CODE: &str = "claude-code";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AssistantProvider {
    #[default]
    LocalModel,
    ClaudeCode,
}

impl AssistantProvider {
    pub fn key(self) -> &'static str {
        match self {
            Self::LocalModel => LOCAL_MODEL,
            Self::ClaudeCode => CLAUDE_CODE,
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        [Self::LocalModel, Self::ClaudeCode]
            .into_iter()
            .find(|provider| provider.key() == key)
    }
}
