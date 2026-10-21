use domovoi_agent::PastTurn;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EarlierTurn {
    pub question: String,
    pub answer: String,
    pub paths: Vec<String>,
}

impl EarlierTurn {
    pub fn past_turn(&self) -> PastTurn {
        PastTurn {
            question: self.question.clone(),
            answer: self.answer.clone(),
            paths: self.paths.clone(),
        }
    }
}
