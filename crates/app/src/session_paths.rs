use std::io;
use std::path::{Path, PathBuf};

use domovoi_dups::CloneJournal;
use domovoi_index::cache_directory;
use domovoi_rules::TrashJournal;

const CACHE_FILE: &str = "scan.cache";
const TRASH_JOURNAL_FILE: &str = "trash-journal";
const CLONE_JOURNAL_FILE: &str = "clone-journal";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPaths {
    pub cache: PathBuf,
    pub trash_journal: PathBuf,
    pub clone_journal: PathBuf,
}

impl SessionPaths {
    pub fn for_user() -> io::Result<Self> {
        Ok(Self {
            cache: cache_directory()?.join(CACHE_FILE),
            trash_journal: TrashJournal::default_path()?,
            clone_journal: CloneJournal::default_path()?,
        })
    }

    pub fn in_directory(directory: &Path) -> Self {
        Self {
            cache: directory.join(CACHE_FILE),
            trash_journal: directory.join(TRASH_JOURNAL_FILE),
            clone_journal: directory.join(CLONE_JOURNAL_FILE),
        }
    }

    pub fn create_directories(&self) -> io::Result<()> {
        for file in [&self.cache, &self.trash_journal, &self.clone_journal] {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }
}
