use domovoi_dups::DuplicateStage;
use serde::Deserialize;

use crate::strings::{STAGE_CLOUD, STAGE_CONTENTS, STAGE_EDGES};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStage {
    Edges,
    Contents,
    CloudFolders,
}

impl SearchStage {
    pub fn title(self) -> &'static str {
        match self {
            Self::Edges => STAGE_EDGES,
            Self::Contents => STAGE_CONTENTS,
            Self::CloudFolders => STAGE_CLOUD,
        }
    }
}

impl From<DuplicateStage> for SearchStage {
    fn from(stage: DuplicateStage) -> Self {
        match stage {
            DuplicateStage::Edges => Self::Edges,
            DuplicateStage::Contents => Self::Contents,
            DuplicateStage::CloudFolders => Self::CloudFolders,
        }
    }
}
