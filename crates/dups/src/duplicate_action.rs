#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DuplicateAction {
    #[default]
    MoveToTrash,
    ReplaceWithClone,
}
