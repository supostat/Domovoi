use domovoi_scanner::Node;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListedEntry {
    Kept(Node),
    Read(Node),
}

impl ListedEntry {
    pub fn node(&self) -> &Node {
        match self {
            Self::Kept(node) | Self::Read(node) => node,
        }
    }
}
