use crate::model_hypothesis::ModelHypothesis;

#[derive(Clone, Debug, PartialEq)]
pub struct FinalAnswer {
    pub answer: String,
    pub hypotheses: Vec<ModelHypothesis>,
}
