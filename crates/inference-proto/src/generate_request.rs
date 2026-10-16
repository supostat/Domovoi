use crate::sampling::Sampling;

#[derive(Clone, Debug, PartialEq)]
pub struct GenerateRequest {
    pub prompt: String,
    pub grammar: Option<String>,
    pub sampling: Sampling,
    pub max_tokens: u32,
}
