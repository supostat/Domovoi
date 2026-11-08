use domovoi_index::{Index, NodeId, Timestamp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemberStamp {
    file_id: u64,
    size: u64,
    modified: Timestamp,
}

impl MemberStamp {
    pub(crate) fn of(index: &Index, member: NodeId) -> Self {
        let node = index.node(member);
        Self {
            file_id: node.file_id,
            size: node.total_size,
            modified: node.modified,
        }
    }
}
