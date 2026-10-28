use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayApplication {
    pub bundle_id: String,
    pub name: String,
    pub name_only: Vec<PathBuf>,
}
