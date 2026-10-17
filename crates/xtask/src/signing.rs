use std::io;
use std::path::Path;
use std::process::Command;

const CODESIGN: &str = "/usr/bin/codesign";
const AD_HOC: &str = "-";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Signing {
    pub(crate) identity: String,
    pub(crate) team: Option<String>,
    pub(crate) hardened: bool,
}

impl Signing {
    pub(crate) fn ad_hoc() -> Self {
        Self {
            identity: AD_HOC.to_owned(),
            team: None,
            hardened: false,
        }
    }

    pub(crate) fn sign(&self, path: &Path, entitlements: Option<&Path>) -> io::Result<()> {
        let mut command = Command::new(CODESIGN);
        command.args(["--force", "--sign", &self.identity]);
        if self.hardened {
            command.args(["--options", "runtime", "--timestamp"]);
        }
        if let Some(entitlements) = entitlements {
            command.arg("--entitlements").arg(entitlements);
        }
        let output = command.arg(path).output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "codesign {} failed: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }
}
