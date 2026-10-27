#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Node(usize),
    Group(usize),
}

impl Hit {
    pub fn node(self) -> Option<usize> {
        match self {
            Self::Node(node) => Some(node),
            Self::Group(_) => None,
        }
    }
}
