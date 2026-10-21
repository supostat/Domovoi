#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PastTurn {
    pub question: String,
    pub answer: String,
    pub paths: Vec<String>,
}
