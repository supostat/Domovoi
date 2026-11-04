use crate::drop_kind::DropKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropHover {
    pub kind: DropKind,
    pub title: String,
}
