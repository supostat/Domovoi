use crate::node_id::NodeId;
use crate::spliced_directory::SplicedDirectory;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeltaApplied {
    Spliced {
        directories: Vec<SplicedDirectory>,
        regrouped: Vec<NodeId>,
    },
    Compacted {
        directories: Vec<NodeId>,
    },
}

impl DeltaApplied {
    pub fn directories(&self) -> Vec<NodeId> {
        match self {
            Self::Spliced { directories, .. } => directories.iter().map(|spliced| spliced.directory).collect(),
            Self::Compacted { directories } => directories.clone(),
        }
    }
}
