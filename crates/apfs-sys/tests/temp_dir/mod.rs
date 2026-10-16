use std::fs::{self, File};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub(crate) fn new(label: &str) -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("domovoi-apfs-sys-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).expect("temp directory is created");
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn open(&self) -> OwnedFd {
        OwnedFd::from(File::open(&self.path).expect("temp directory opens"))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!("temp directory {} was not removed: {error}", self.path.display());
        }
    }
}
