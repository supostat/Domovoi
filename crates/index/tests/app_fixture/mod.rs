#![allow(dead_code)]

use std::fs;
use std::path::Path;

use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_scanner::{ScanOptions, scan};

pub(crate) fn volume_root(root: &Path) -> IndexRoot {
    IndexRoot::Volume {
        path: fs::canonicalize(root)
            .expect("the root resolves")
            .to_string_lossy()
            .into_owned(),
    }
}

pub(crate) fn volume_index(root: &Path) -> Index {
    let report = scan(root, &ScanOptions::default()).expect("the tree scans");
    Index::build(&report, volume_root(root), &Enrichment::default())
}
