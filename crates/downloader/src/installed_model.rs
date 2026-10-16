use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledModel {
    pub entry_id: String,
    pub path: PathBuf,
}
