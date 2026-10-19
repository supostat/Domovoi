use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use domovoi_apfs_sys::sys::UF_COMPRESSED;
use domovoi_apfs_sys::{link_id, private_size, rename_exclusive, write_atomically};
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_scanner::{ScanOptions, scan};
use objc2_foundation::{NSFileManager, NSString, NSURL};
use rkyv::rancor;

use crate::trash_failure::TrashFailure;
use crate::trash_item::trash_item;
use crate::trash_subject::TrashSubject;

const JOURNAL_PATH: &str = "Library/Application Support/com.domovoi.app/trash-journal";

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct JournalEntry {
    pub original: String,
    pub original_physical: String,
    pub trashed: String,
    pub link_id: u64,
    pub alloc_size: u64,
    pub flags: u32,
    pub clone_id: u64,
    pub rule_id: Option<String>,
    pub trashed_at: u64,
}

#[derive(Debug)]
pub enum RestoreError {
    NotInTrash,
    Replaced,
    ParentMissing,
    Io(io::Error),
}

#[derive(Debug)]
pub struct TrashJournal {
    path: PathBuf,
    entries: Vec<JournalEntry>,
}

impl TrashJournal {
    pub fn default_path() -> io::Result<PathBuf> {
        env::home_dir()
            .map(|home| home.join(JOURNAL_PATH))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        let entries = match fs::read(path) {
            Ok(bytes) => {
                let mut aligned = rkyv::util::AlignedVec::<16>::new();
                aligned.extend_from_slice(&bytes);
                rkyv::from_bytes::<Vec<JournalEntry>, rancor::Error>(&aligned)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            path: path.to_path_buf(),
            entries,
        })
    }

    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    pub fn trash(&mut self, subject: &TrashSubject) -> io::Result<Result<PathBuf, TrashFailure>> {
        let trashed = match trash_item(&subject.location)? {
            Ok(trashed) => named_like_original(&trashed, &subject.original),
            Err(failure) => return Ok(Err(failure)),
        };
        self.entries.push(JournalEntry {
            original: subject.original.to_string_lossy().into_owned(),
            original_physical: subject.original_physical.to_string_lossy().into_owned(),
            trashed: trashed.to_string_lossy().into_owned(),
            link_id: subject.link_id,
            alloc_size: subject.alloc_size,
            flags: subject.flags,
            clone_id: subject.clone_id,
            rule_id: subject.rule_id.clone(),
            trashed_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        });
        self.save()?;
        Ok(Ok(trashed))
    }

    pub fn forget_emptied(&mut self) -> io::Result<()> {
        self.entries
            .retain(|entry| Path::new(&entry.trashed).symlink_metadata().is_ok());
        self.save()
    }

    pub fn reclaimable_after_emptying(&self) -> io::Result<u64> {
        let mut total = 0;
        for entry in &self.entries {
            let trashed = Path::new(&entry.trashed);
            let Ok(metadata) = trashed.symlink_metadata() else {
                continue;
            };
            total += if metadata.is_dir() {
                let report = scan(trashed, &ScanOptions::default()).map_err(io::Error::other)?;
                let index = Index::build(&report, IndexRoot::for_path(trashed)?, &Enrichment::default());
                index.reclaimable(index.root())?.bytes
            } else if entry.flags & UF_COMPRESSED != 0 {
                entry.alloc_size
            } else {
                private_size(trashed)?
            };
        }
        Ok(total)
    }

    pub fn restore(&mut self, position: usize) -> Result<PathBuf, RestoreError> {
        let entry = self.entries[position].clone();
        let trashed = Path::new(&entry.trashed);
        let current = link_id(trashed).map_err(|error| match error.raw_os_error() {
            Some(libc::ENOENT) => RestoreError::NotInTrash,
            _ => RestoreError::Io(error),
        })?;
        if current != entry.link_id {
            return Err(RestoreError::Replaced);
        }
        let original = Path::new(&entry.original_physical);
        let parent = original.parent().ok_or(RestoreError::ParentMissing)?;
        if !parent.is_dir() {
            return Err(RestoreError::ParentMissing);
        }
        let destination = free_name(original);
        let manager = NSFileManager::defaultManager();
        manager
            .moveItemAtURL_toURL_error(&*file_url(trashed)?, &*file_url(&destination)?)
            .map_err(|error| RestoreError::Io(io::Error::other(error.localizedDescription().to_string())))?;
        self.entries.remove(position);
        self.save().map_err(RestoreError::Io)?;
        Ok(destination)
    }

    fn save(&self) -> io::Result<()> {
        let bytes = rkyv::to_bytes::<rancor::Error>(&self.entries).map_err(io::Error::other)?;
        write_atomically(&self.path, &bytes)
    }
}

pub(crate) fn file_url(path: &Path) -> Result<objc2::rc::Retained<NSURL>, RestoreError> {
    let text = path
        .to_str()
        .ok_or_else(|| RestoreError::Io(io::Error::new(io::ErrorKind::InvalidInput, "path is not UTF-8")))?;
    Ok(NSURL::fileURLWithPath(&NSString::from_str(text)))
}

fn free_name(original: &Path) -> PathBuf {
    let parent = original.parent().unwrap_or_else(|| Path::new("/"));
    let Some(name) = original.file_name() else {
        return original.to_path_buf();
    };
    finder_names(name)
        .map(|candidate| parent.join(candidate))
        .find(|candidate| candidate.symlink_metadata().is_err())
        .expect("finder names never run out")
}

fn named_like_original(trashed: &Path, original: &Path) -> PathBuf {
    let (Some(name), Some(wanted), Some(folder)) = (trashed.file_name(), original.file_name(), trashed.parent()) else {
        return trashed.to_path_buf();
    };
    if name == wanted {
        return trashed.to_path_buf();
    }
    let Ok(directory) = File::open(folder) else {
        return trashed.to_path_buf();
    };
    for candidate in finder_names(wanted) {
        match rename_exclusive(directory.as_fd(), name, directory.as_fd(), &candidate) {
            Ok(()) => return folder.join(candidate),
            Err(error) if error.raw_os_error() == Some(libc::EEXIST) => {}
            Err(_) => return trashed.to_path_buf(),
        }
    }
    unreachable!("finder names never run out")
}

fn finder_names(name: &OsStr) -> impl Iterator<Item = OsString> {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned());
    std::iter::once(name.to_os_string()).chain((2_u64..).map(move |number| {
        OsString::from(match &extension {
            Some(extension) => format!("{stem} {number}.{extension}"),
            None => format!("{stem} {number}"),
        })
    }))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::free_name;

    #[test]
    fn collisions_take_the_next_finder_number() {
        let directory = std::env::temp_dir().join(format!("domovoi-free-name-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("directory is created");
        fs::write(directory.join("report.pdf"), b"a").expect("file is written");
        fs::write(directory.join("report 2.pdf"), b"b").expect("file is written");
        assert_eq!(free_name(&directory.join("report.pdf")), directory.join("report 3.pdf"));
        assert_eq!(free_name(&directory.join("fresh.txt")), directory.join("fresh.txt"));
        fs::remove_dir_all(&directory).expect("directory is removed");
    }
}
