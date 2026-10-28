use domovoi_rules::DeleteProgress;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteStatus {
    pub files_done: usize,
    pub files_total: usize,
}

impl From<DeleteProgress> for DeleteStatus {
    fn from(progress: DeleteProgress) -> Self {
        Self {
            files_done: progress.files_done,
            files_total: progress.files_total,
        }
    }
}
