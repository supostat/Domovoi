use domovoi_agent::ToolStep;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AskEvent {
    Loaded,
    Step(ToolStep),
}
