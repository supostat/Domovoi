use std::fs;
use std::io;
use std::path::Path;

use domovoi_scanner::{ScanError, ScanOptions, scan};

use crate::enrichment::Enrichment;
use crate::index::Index;
use crate::node_kind::NodeKind;

impl Index {
    pub fn rescan(&mut self, changed: &Path, options: &ScanOptions, enrichment: &Enrichment) -> Result<(), ScanError> {
        let mut target = changed.to_path_buf();
        let directory = loop {
            if let Some(node) = self
                .find(&target)
                .filter(|node| self.node(*node).kind != NodeKind::File)
                .filter(|node| fs::symlink_metadata(self.physical_path(*node)).is_ok())
            {
                break node;
            }
            if !target.pop() {
                return Err(ScanError::Root(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{} is outside the index", changed.display()),
                )));
            }
        };
        let report = scan(&self.physical_path(directory), options)?;
        self.replace_children(directory, &report.root);
        self.derive(enrichment);
        Ok(())
    }
}
