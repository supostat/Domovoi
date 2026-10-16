use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::os::fd::AsFd;
use std::path::Path;

use domovoi_apfs_sys::{ftruncate, get_xattr, pwrite_at, set_xattr};

use crate::install_error::InstallError;
use crate::sha256_digest::Sha256Digest;
use crate::sha256_hasher::Sha256Hasher;

const VALIDATOR_ATTRIBUTE: &str = "com.domovoi.download-validator";

#[derive(Debug)]
pub(crate) struct PartialFile {
    file: File,
    received: u64,
    hasher: Sha256Hasher,
    validator: Option<String>,
}

impl PartialFile {
    pub(crate) fn open(path: &Path, size: u64) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let mut partial = Self {
            received: file.metadata()?.len(),
            validator: get_xattr(file.as_fd(), OsStr::new(VALIDATOR_ATTRIBUTE))?
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .filter(|validator| !validator.is_empty()),
            hasher: Sha256Hasher::default(),
            file,
        };
        if partial.received > size {
            partial.restart()?;
        } else {
            let mut reader = &partial.file;
            reader.seek(SeekFrom::Start(0))?;
            partial.hasher.update_from_reader(&mut reader, partial.received)?;
        }
        Ok(partial)
    }

    pub(crate) fn received(&self) -> u64 {
        self.received
    }

    pub(crate) fn validator(&self) -> Option<&str> {
        self.validator.as_deref()
    }

    pub(crate) fn restart(&mut self) -> io::Result<()> {
        ftruncate(self.file.as_fd(), 0)?;
        self.received = 0;
        self.hasher = Sha256Hasher::default();
        self.validator = None;
        set_xattr(self.file.as_fd(), OsStr::new(VALIDATOR_ATTRIBUTE), b"")
    }

    pub(crate) fn remember_validator(&mut self, validator: &str) -> io::Result<()> {
        if self.validator.is_none() {
            set_xattr(self.file.as_fd(), OsStr::new(VALIDATOR_ATTRIBUTE), validator.as_bytes())?;
            self.validator = Some(validator.to_owned());
        }
        Ok(())
    }

    pub(crate) fn append(&mut self, bytes: &[u8], expected_size: u64) -> Result<(), InstallError> {
        let after = self.received + bytes.len() as u64;
        if after > expected_size {
            return Err(InstallError::SizeMismatch {
                expected: expected_size,
                received: after,
            });
        }
        pwrite_at(self.file.as_fd(), bytes, self.received)?;
        self.hasher.update(bytes);
        self.received = after;
        Ok(())
    }

    pub(crate) fn verify(mut self, expected: Sha256Digest) -> Result<(), InstallError> {
        self.file.sync_all()?;
        let hasher = std::mem::take(&mut self.hasher);
        let received = hasher.finish();
        if received != expected {
            self.restart()?;
            return Err(InstallError::DigestMismatch { expected, received });
        }
        Ok(())
    }
}
