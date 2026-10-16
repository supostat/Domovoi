use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

const CURL: &str = "/usr/bin/curl";
const SHASUM: &str = "/usr/bin/shasum";
const PARTIAL_EXTENSION: &str = "partial";

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
        let path = self.path(root);
        match fs::metadata(&path) {
            Ok(metadata) => Ok(metadata.len() == self.size && sha256(&path)? == self.sha256),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn download(&self, root: &Path) -> io::Result<()> {
        let url = self.url.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} is committed and does not match the manifest", self.name),
            )
        })?;
        let path = self.path(root);
        let directory = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a model location has no directory"))?;
        fs::create_dir_all(directory)?;
        let partial = path.with_extension(PARTIAL_EXTENSION);
        let status = Command::new(CURL)
            .args(["--fail", "--location", "--silent", "--show-error", "--output"])
            .arg(&partial)
            .arg(url)
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!("downloading {url} failed with {status}")));
        }
        let received = sha256(&partial)?;
        if received != self.sha256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} has SHA-256 {received}, the manifest expects {}",
                    self.name, self.sha256
                ),
            ));
        }
        fs::rename(partial, path)
    }
}

fn sha256(path: &Path) -> io::Result<String> {
    let output = Command::new(SHASUM).args(["-a", "256"]).arg(path).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!("shasum failed for {}", path.display())));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "shasum printed non-UTF-8 output"))?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "shasum printed no digest"))
}
