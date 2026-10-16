use domovoi_index::NodeId;

use crate::skip_reason::SkipReason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkippedFile {
    pub node: NodeId,
    pub reason: SkipReason,
}
