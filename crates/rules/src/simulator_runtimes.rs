use std::collections::BTreeSet;
use std::path::Path;

use std::process::Command;

const DEVICE_PLIST: &str = "device.plist";
const RUNTIME_KEY: &str = "runtime";
const BUNDLE_IDENTIFIER_KEY: &str = "CFBundleIdentifier";
const RUNTIME_DIRECTORIES: [&str; 2] = [
    "/Library/Developer/CoreSimulator/Profiles/Runtimes",
    "/Library/Developer/CoreSimulator/Volumes",
];
const VOLUME_RUNTIMES: &str = "Library/Developer/CoreSimulator/Profiles/Runtimes";
const RUNTIME_INFO: &str = "Contents/Info.plist";
const PLUTIL: &str = "/usr/bin/plutil";

pub trait SimulatorRuntimes: Send + Sync {
    fn device_runtime_missing(&self, device: &Path) -> bool;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstalledSimulatorRuntimes {
    identifiers: BTreeSet<String>,
}

impl InstalledSimulatorRuntimes {
    pub fn new(identifiers: BTreeSet<String>) -> Self {
        Self { identifiers }
    }

    pub fn discover() -> Self {
        let mut identifiers = BTreeSet::new();
        for directory in RUNTIME_DIRECTORIES {
            for entry in children(Path::new(directory)) {
                let runtimes = if entry.extension().is_some_and(|extension| extension == "simruntime") {
                    vec![entry]
                } else {
                    children(&entry.join(VOLUME_RUNTIMES))
                };
                identifiers.extend(
                    runtimes
                        .iter()
                        .filter_map(|runtime| plist_string(&runtime.join(RUNTIME_INFO), BUNDLE_IDENTIFIER_KEY)),
                );
            }
        }
        Self { identifiers }
    }
}

impl SimulatorRuntimes for InstalledSimulatorRuntimes {
    fn device_runtime_missing(&self, device: &Path) -> bool {
        plist_string(&device.join(DEVICE_PLIST), RUNTIME_KEY)
            .is_some_and(|runtime| !self.identifiers.contains(&runtime))
    }
}

fn children(directory: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(directory)
        .map(|entries| entries.filter_map(Result::ok).map(|entry| entry.path()).collect())
        .unwrap_or_default()
}

fn plist_string(path: &Path, key: &str) -> Option<String> {
    let output = Command::new(PLUTIL)
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(path)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}
