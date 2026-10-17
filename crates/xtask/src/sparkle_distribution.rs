use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::verified_download::{download_verified, is_verified};

const VERSION: &str = "2.10.0";
const ARCHIVE: &str = "Sparkle-2.10.0.tar.xz";
const URL: &str = "https://github.com/sparkle-project/Sparkle/releases/download/2.10.0/Sparkle-2.10.0.tar.xz";
const SIZE: u64 = 16_319_840;
const SHA256: &str = "c2bf58aa8387266ac179357b1415d6f2635f044da8be41042af32425dae6da0c";
const TAR: &str = "/usr/bin/tar";
const FRAMEWORK: &str = "Sparkle.framework";
const LICENSE: &str = "LICENSE";

#[derive(Clone, Debug)]
pub(crate) struct SparkleDistribution {
    directory: PathBuf,
}

impl SparkleDistribution {
    pub(crate) fn ensure(root: &Path) -> io::Result<Self> {
        let cache = root.join("target/sparkle");
        let distribution = Self {
            directory: cache.join(VERSION),
        };
        if distribution.framework().join("Versions/B/Sparkle").is_file() {
            return Ok(distribution);
        }
        let archive = cache.join(ARCHIVE);
        if !is_verified(&archive, SIZE, SHA256)? {
            download_verified(URL, &archive, SIZE, SHA256)?;
        }
        fs::create_dir_all(&distribution.directory)?;
        let status = Command::new(TAR)
            .arg("-xJf")
            .arg(&archive)
            .arg("-C")
            .arg(&distribution.directory)
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!("unpacking {ARCHIVE} failed with {status}")));
        }
        Ok(distribution)
    }

    pub(crate) fn framework(&self) -> PathBuf {
        self.directory.join(FRAMEWORK)
    }

    pub(crate) fn license(&self) -> PathBuf {
        self.directory.join(LICENSE)
    }

    pub(crate) fn tool(&self, name: &str) -> PathBuf {
        self.directory.join("bin").join(name)
    }
}
