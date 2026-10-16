use std::path::{Path, PathBuf};

const HOME_FOLDERS: [&str; 5] = [
    "Desktop",
    "Documents",
    "Downloads",
    "Library/Mobile Documents",
    "Library/CloudStorage",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProtectedFolders {
    paths: Vec<PathBuf>,
}

impl ProtectedFolders {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }

    pub fn of_home(home: &Path) -> Self {
        Self::new(HOME_FOLDERS.iter().map(|folder| home.join(folder)).collect())
    }

    pub(crate) fn contains(&self, canonical: &Path) -> bool {
        self.paths.iter().any(|path| path == canonical)
    }
}
