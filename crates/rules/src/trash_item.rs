use std::io;
use std::path::{Path, PathBuf};

use objc2_foundation::NSFileManager;

use crate::trash_failure::TrashFailure;
use crate::trash_journal::file_url;

pub fn trash_item(location: &Path) -> io::Result<Result<PathBuf, TrashFailure>> {
    let url = file_url(location).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path is not UTF-8"))?;
    let mut resulting = None;
    match NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting)) {
        Ok(()) => resulting
            .and_then(|url| url.path())
            .map(|path| Ok(PathBuf::from(path.to_string())))
            .ok_or_else(|| io::Error::other("Finder did not report where the item went")),
        Err(error) => Ok(Err(TrashFailure::from_cocoa(
            error.code(),
            error.localizedDescription().to_string(),
        ))),
    }
}
