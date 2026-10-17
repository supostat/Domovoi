use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::verified_download::{download_verified, is_verified};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TestModel {
    pub(crate) name: String,
    location: PathBuf,
    url: Option<String>,
    size: u64,
    sha256: String,
    origin: String,
    license: String,
}

impl TestModel {
    pub(crate) fn provenance(&self) -> String {
        format!("{}; license: {}", self.origin, self.license)
    }

    pub(crate) fn path(&self, root: &Path) -> PathBuf {
        root.join(&self.location)
    }

    pub(crate) fn is_present(&self, root: &Path) -> io::Result<bool> {
        is_verified(&self.path(root), self.size, &self.sha256)
    }

    pub(crate) fn download(&self, root: &Path) -> io::Result<()> {
        let url = self.url.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} is committed and does not match the manifest", self.name),
            )
        })?;
        download_verified(url, &self.path(root), self.size, &self.sha256)
    }
}
