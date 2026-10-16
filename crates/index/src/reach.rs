use domovoi_scanner::NodeState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum Reach {
    Leaf,
    Listed,
    OtherVolume,
    Firmlink,
    HiddenByPrivacy,
    DataVault,
    OtherUser,
    InCloud,
    Deferred,
    Vanished,
    Unreadable(i32),
}

impl Reach {
    pub fn hides_content(self) -> bool {
        matches!(
            self,
            Self::HiddenByPrivacy | Self::DataVault | Self::OtherUser | Self::Deferred | Self::Unreadable(_)
        )
    }
}

impl From<NodeState> for Reach {
    fn from(state: NodeState) -> Self {
        match state {
            NodeState::Leaf => Self::Leaf,
            NodeState::Listed => Self::Listed,
            NodeState::OtherVolume => Self::OtherVolume,
            NodeState::Firmlink => Self::Firmlink,
            NodeState::HiddenByPrivacy => Self::HiddenByPrivacy,
            NodeState::DataVault => Self::DataVault,
            NodeState::OtherUser => Self::OtherUser,
            NodeState::InCloud => Self::InCloud,
            NodeState::Deferred => Self::Deferred,
            NodeState::Vanished => Self::Vanished,
            NodeState::Unreadable(errno) => Self::Unreadable(errno),
        }
    }
}
