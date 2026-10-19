use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedPath {
    pub path: PathBuf,
    pub subtree: bool,
}
