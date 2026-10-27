#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    Node,
    SmallGroup { count: usize, bytes: u64 },
}
