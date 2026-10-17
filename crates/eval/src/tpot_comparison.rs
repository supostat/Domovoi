use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TpotComparison {
    pub with_grammar_seconds: f64,
    pub without_grammar_seconds: f64,
}

impl TpotComparison {
    pub fn ratio(&self) -> f64 {
        self.with_grammar_seconds / self.without_grammar_seconds
    }
}
