use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::ffi::spotlight_last_used;
use crate::installed_applications::InstalledApplications;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Enrichment {
    pub spotlight_last_used: Vec<(PathBuf, SystemTime)>,
    pub applications: InstalledApplications,
    pub model_paths: Vec<PathBuf>,
}

impl Enrichment {
    pub fn gather(model_paths: Vec<PathBuf>) -> io::Result<Self> {
        Ok(Self {
            spotlight_last_used: spotlight_last_used()?,
            applications: InstalledApplications::discover()?,
            model_paths,
        })
    }
}
