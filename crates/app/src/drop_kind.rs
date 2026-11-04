#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropKind {
    Add,
    Denied,
    Already,
}
