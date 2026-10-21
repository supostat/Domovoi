use domovoi_agent::{FocusContext, PastTurn};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Question {
    pub text: String,
    pub history: Vec<PastTurn>,
    pub focus: FocusContext,
}
