use std::fs;
use std::io;

use serde::Deserialize;

use crate::finding::Finding;
use crate::test_model::TestModel;
use crate::workspace;

const MANIFEST: &str = "assets/test/manifest.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    models: Vec<TestModel>,
}

pub(crate) fn run() -> io::Result<Vec<Finding>> {
    let root = workspace::root();
    let manifest: Manifest = serde_json::from_str(&fs::read_to_string(root.join(MANIFEST))?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{MANIFEST}: {error}")))?;
    for model in &manifest.models {
        if model.is_present(&root)? {
            println!("{} is present", model.name);
            continue;
        }
        model.download(&root)?;
        println!(
            "{} is downloaded to {} ({})",
            model.name,
            model.path(&root).display(),
            model.provenance()
        );
    }
    Ok(Vec::new())
}
