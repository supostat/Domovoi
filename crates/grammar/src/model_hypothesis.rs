#[derive(Clone, Debug, PartialEq)]
pub struct ModelHypothesis {
    pub path: String,
    pub guess: String,
    pub confidence: f64,
}
