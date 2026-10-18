use std::env;
use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::{mmap_readonly, os_build_version, volume_uuid};
use domovoi_scanner::history_store;
use rkyv::rancor;

use crate::index::Index;

const MAGIC: [u8; 8] = *b"DOMOVOI\0";
const FORMAT_VERSION: u32 = 1;
const CACHE_DIRECTORY: &str = "Library/Caches/com.domovoi.app";
const TEMPORARY_SUFFIX: &str = "partial";

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct CacheHeader {
    pub magic: [u8; 8],
    pub format: u32,
    pub volume_uuid: String,
    pub event_store: Option<String>,
    pub os_build: String,
    pub app_version: String,
}

impl CacheHeader {
    pub fn current(data_root: &Path, app_version: &str) -> io::Result<Self> {
        Ok(Self {
            magic: MAGIC,
            format: FORMAT_VERSION,
            volume_uuid: volume_uuid(data_root)?,
            event_store: history_store(data_root)?,
            os_build: os_build_version()?,
            app_version: app_version.to_owned(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CachedIndex {
    pub index: Index,
    pub last_event_id: u64,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
struct CacheFile<'source> {
    #[rkyv(with = rkyv::with::Inline)]
    header: &'source CacheHeader,
    last_event_id: u64,
    #[rkyv(with = rkyv::with::Inline)]
    index: &'source Index,
}

#[derive(Debug)]
pub enum CacheError {
    Io(io::Error),
    Corrupt(String),
    Mismatch,
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "scan cache is unreadable: {error}"),
            Self::Corrupt(reason) => write!(formatter, "scan cache is corrupt: {reason}"),
            Self::Mismatch => {
                formatter.write_str("scan cache belongs to another volume, system or application version")
            }
        }
    }
}

impl Error for CacheError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Corrupt(_) | Self::Mismatch => None,
        }
    }
}

pub fn cache_directory() -> io::Result<PathBuf> {
    env::home_dir()
        .map(|home| home.join(CACHE_DIRECTORY))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))
}

pub fn write_cache(path: &Path, header: &CacheHeader, last_event_id: u64, index: &Index) -> io::Result<()> {
    let file = CacheFile {
        header,
        last_event_id,
        index,
    };
    let bytes = rkyv::to_bytes::<rancor::Error>(&file).map_err(io::Error::other)?;
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cache path has no directory"))?;
    fs::create_dir_all(directory)?;
    let temporary = path.with_extension(TEMPORARY_SUFFIX);
    let mut output = File::create(&temporary)?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    fs::rename(&temporary, path)
}

pub fn open_cache(path: &Path, expected: &CacheHeader) -> Result<CachedIndex, CacheError> {
    let mapped = mmap_readonly(path).map_err(CacheError::Io)?;
    let archived = rkyv::access::<ArchivedCacheFile, rancor::Error>(&mapped)
        .map_err(|error| CacheError::Corrupt(error.to_string()))?;
    let header = rkyv::deserialize::<CacheHeader, rancor::Error>(&archived.header)
        .map_err(|error| CacheError::Corrupt(error.to_string()))?;
    if header != *expected {
        return Err(CacheError::Mismatch);
    }
    let index = rkyv::deserialize::<Index, rancor::Error>(&archived.index)
        .map_err(|error| CacheError::Corrupt(error.to_string()))?;
    Ok(CachedIndex {
        index,
        last_event_id: archived.last_event_id.to_native(),
    })
}
