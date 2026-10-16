use std::path::{Path, PathBuf};

const HOME_CLOUD_FOLDERS: [&str; 2] = ["Library/Mobile Documents", "Library/CloudStorage"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudFolders {
    roots: Vec<PathBuf>,
}

impl CloudFolders {
    pub fn of_home(home: &Path) -> Self {
        Self::new(HOME_CLOUD_FOLDERS.iter().map(|folder| home.join(folder)).collect())
    }

    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    pub fn contains(&self, canonical: &Path) -> bool {
        self.roots.iter().any(|root| canonical.starts_with(root))
    }
}
