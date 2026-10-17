use domovoi_index::{NodeId, Source};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOrigin {
    Indexed(NodeId),
    Measured(Source),
    Outline,
}
