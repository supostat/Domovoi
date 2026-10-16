use std::fs::{self, File};
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use domovoi_index::ModelEvent;
use domovoi_rules::trash_item;
use objc2_foundation::{NSFileManager, NSSearchPathDirectory, NSSearchPathDomainMask};

use crate::backup_exclusion::exclude_from_backup;
use crate::catalog::Catalog;
use crate::catalog_entry::CatalogEntry;
use crate::install_error::InstallError;
use crate::installed_model::InstalledModel;
use crate::orphan::Orphan;
use crate::orphan_reason::OrphanReason;
use crate::partial_download::PartialDownload;
use crate::sha256_hasher::Sha256Hasher;
use crate::store_report::StoreReport;

const MODELS_DIRECTORY: [&str; 2] = ["Domovoi", "models"];
const PARTIAL_SUFFIX: &str = ".partial";
const SPACE_RESERVE_DIVISOR: u64 = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelStore {
    directory: PathBuf,
}

impl ModelStore {
    pub fn default_directory() -> io::Result<PathBuf> {
        let support = NSFileManager::defaultManager()
            .URLsForDirectory_inDomains(
                NSSearchPathDirectory::ApplicationSupportDirectory,
                NSSearchPathDomainMask::UserDomainMask,
            )
            .firstObject()
            .and_then(|url| url.path())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the user has no Application Support folder"))?;
        Ok(MODELS_DIRECTORY
            .iter()
            .fold(PathBuf::from(support.to_string()), |path, component| {
                path.join(component)
            }))
    }

    pub fn open(directory: &Path) -> io::Result<Self> {
        if !directory.is_dir() {
            if let Some(parent) = directory.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::create_dir(directory)?;
            exclude_from_backup(directory)?;
        }
        Ok(Self {
            directory: directory.to_path_buf(),
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn model_path(&self, entry: &CatalogEntry) -> PathBuf {
        self.directory.join(&entry.file)
    }

    pub fn partial_path(&self, entry: &CatalogEntry) -> PathBuf {
        self.directory.join(format!("{}{PARTIAL_SUFFIX}", entry.file))
    }

    pub fn available_bytes(&self) -> io::Result<u64> {
        let path = std::ffi::CString::new(self.directory.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the path contains NUL"))?;
        let mut statistics = MaybeUninit::<libc::statfs>::uninit();
        if unsafe { libc::statfs(path.as_ptr(), statistics.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let statistics = unsafe { statistics.assume_init() };
        Ok(statistics.f_bavail * u64::from(statistics.f_bsize))
    }

    pub fn ensure_space(&self, entry: &CatalogEntry, already_received: u64) -> Result<(), InstallError> {
        let required = entry.size.saturating_sub(already_received) + entry.size / SPACE_RESERVE_DIVISOR;
        let available = self.available_bytes()?;
        if available < required {
            return Err(InstallError::InsufficientSpace { required, available });
        }
        Ok(())
    }

    pub fn scan(&self, catalog: &Catalog) -> io::Result<StoreReport> {
        let mut report = StoreReport::default();
        let mut names: Vec<(String, PathBuf)> = Vec::new();
        for item in fs::read_dir(&self.directory)? {
            let item = item?;
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !item.file_type()?.is_file() {
                continue;
            }
            names.push((name, item.path()));
        }
        names.sort();
        for (name, path) in names {
            if let Some(entry) = catalog.entries.iter().find(|entry| entry.file == name) {
                if matches_entry(&path, entry)? {
                    report.installed.push(InstalledModel {
                        entry_id: entry.id.clone(),
                        path,
                    });
                } else {
                    report.orphans.push(Orphan {
                        path,
                        reason: OrphanReason::DigestMismatch,
                    });
                }
            } else if let Some(entry) = catalog
                .entries
                .iter()
                .find(|entry| name.strip_suffix(PARTIAL_SUFFIX) == Some(entry.file.as_str()))
            {
                report.partials.push(PartialDownload {
                    entry_id: entry.id.clone(),
                    received: fs::metadata(&path)?.len(),
                    path,
                });
            } else {
                report.orphans.push(Orphan {
                    path,
                    reason: OrphanReason::NotInCatalog,
                });
            }
        }
        Ok(report)
    }

    pub fn adopt(&self, source: &Path, entry: &CatalogEntry) -> Result<ModelEvent, InstallError> {
        let received = fs::metadata(source)?.len();
        if received != entry.size {
            return Err(InstallError::SizeMismatch {
                expected: entry.size,
                received,
            });
        }
        let digest = Sha256Hasher::digest_of_file(source)?;
        if digest != entry.sha256 {
            return Err(InstallError::DigestMismatch {
                expected: entry.sha256,
                received: digest,
            });
        }
        let target = self.model_path(entry);
        let already_in_place = target.exists() && fs::canonicalize(source)? == fs::canonicalize(&target)?;
        if !already_in_place {
            let partial = self.partial_path(entry);
            fs::copy(source, &partial)?;
            File::open(&partial)?.sync_all()?;
            fs::rename(&partial, &target)?;
        }
        Ok(ModelEvent::Downloaded { path: target })
    }

    pub fn remove(&self, path: &Path) -> Result<ModelEvent, InstallError> {
        self.move_to_trash(path)?;
        Ok(ModelEvent::Removed {
            path: path.to_path_buf(),
        })
    }

    pub(crate) fn install_partial(&self, entry: &CatalogEntry) -> io::Result<ModelEvent> {
        let target = self.model_path(entry);
        fs::rename(self.partial_path(entry), &target)?;
        Ok(ModelEvent::Downloaded { path: target })
    }

    pub(crate) fn move_to_trash(&self, path: &Path) -> Result<(), InstallError> {
        if path.parent() != Some(self.directory.as_path()) {
            return Err(InstallError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a file of the model store", path.display()),
            )));
        }
        trash_item(path)?.map_err(InstallError::Trash)?;
        Ok(())
    }
}

fn matches_entry(path: &Path, entry: &CatalogEntry) -> io::Result<bool> {
    Ok(fs::metadata(path)?.len() == entry.size && Sha256Hasher::digest_of_file(path)? == entry.sha256)
}
