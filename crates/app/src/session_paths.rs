use std::io;
use std::path::{Path, PathBuf};

use domovoi_downloader::ModelStore;
use domovoi_dups::CloneJournal;
use domovoi_index::cache_directory;

const CACHE_FILE: &str = "scan.cache";
const CLONE_JOURNAL_FILE: &str = "clone-journal";
const MODELS_DIRECTORY: &str = "models";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPaths {
    pub cache: PathBuf,
    pub clone_journal: PathBuf,
    pub models: PathBuf,
    pub home: PathBuf,
}

impl SessionPaths {
    pub fn for_user() -> io::Result<Self> {
        Ok(Self {
            cache: cache_directory()?.join(CACHE_FILE),
            clone_journal: CloneJournal::default_path()?,
            models: ModelStore::default_directory()?,
            home: std::env::home_dir()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))?,
        })
    }

    pub fn in_directory(directory: &Path) -> Self {
        Self {
            cache: directory.join(CACHE_FILE),
            clone_journal: directory.join(CLONE_JOURNAL_FILE),
            models: directory.join(MODELS_DIRECTORY),
            home: directory.to_path_buf(),
        }
    }

    #[must_use]
    pub fn with_home(self, home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            ..self
        }
    }

    pub fn create_directories(&self) -> io::Result<()> {
        for file in [&self.cache, &self.clone_journal] {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }
}
