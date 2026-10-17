use std::env;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::bundle_layout::{APP_EXECUTABLE, SERVICE_EXECUTABLE};

const DIAGRAM_METALLIB_VARIABLE: &str = "DOMOVOI_DIAGRAM_METALLIB";
const METALLIB_DIRECTORY_VARIABLE: &str = "DOMOVOI_METALLIB_DIR";

#[derive(Debug, Default)]
pub(crate) struct BuildOutputs {
    pub(crate) app: Option<PathBuf>,
    pub(crate) service: Option<PathBuf>,
    pub(crate) diagram_metallib: Option<PathBuf>,
    pub(crate) metallib_directory: Option<PathBuf>,
}

impl BuildOutputs {
    pub(crate) fn build_release(root: &Path, features: Option<&str>) -> io::Result<Self> {
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
        command
            .current_dir(root)
            .args(["build", "--release", "--message-format=json-render-diagnostics"])
            .args(["--bin", APP_EXECUTABLE, "--bin", SERVICE_EXECUTABLE]);
        if let Some(features) = features {
            command.args(["--features", features]);
        }
        let mut child = command.stdout(Stdio::piped()).spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("cargo has no output"))?;
        let mut outputs = Self::default();
        for line in BufReader::new(stdout).lines() {
            if let Ok(message) = serde_json::from_str::<Value>(&line?) {
                outputs.absorb(&message);
            }
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::other(format!("cargo build --release failed with {status}")));
        }
        Ok(outputs)
    }

    pub(crate) fn require(path: Option<&PathBuf>, what: &str) -> io::Result<PathBuf> {
        path.cloned()
            .ok_or_else(|| io::Error::other(format!("the release build did not report {what}")))
    }

    fn absorb(&mut self, message: &Value) {
        match message["reason"].as_str() {
            Some("compiler-artifact") => {
                let Some(executable) = message["executable"].as_str().map(PathBuf::from) else {
                    return;
                };
                match message["target"]["name"].as_str() {
                    Some(APP_EXECUTABLE) => self.app = Some(executable),
                    Some(SERVICE_EXECUTABLE) => self.service = Some(executable),
                    _ => {}
                }
            }
            Some("build-script-executed") => {
                for pair in message["env"].as_array().into_iter().flatten() {
                    match (pair[0].as_str(), pair[1].as_str()) {
                        (Some(DIAGRAM_METALLIB_VARIABLE), Some(path)) => self.diagram_metallib = Some(path.into()),
                        (Some(METALLIB_DIRECTORY_VARIABLE), Some(path)) => self.metallib_directory = Some(path.into()),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}
