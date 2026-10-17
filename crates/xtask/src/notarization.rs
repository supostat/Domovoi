use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bundle_profile::{BundleProfile, required};

const PROFILE_VARIABLE: &str = "DOMOVOI_NOTARY_PROFILE";
const DITTO: &str = "/usr/bin/ditto";
const XCRUN: &str = "/usr/bin/xcrun";
const HDIUTIL: &str = "/usr/bin/hdiutil";
const VOLUME_NAME: &str = "DOMOVOI";
const ACCEPTED: &str = "status: Accepted";
const DISTRIBUTION_DIRECTORY: &str = "dist";

#[derive(Debug)]
pub(crate) struct Published {
    pub(crate) zip: PathBuf,
    pub(crate) disk_image: PathBuf,
}

pub(crate) fn require_credentials() -> io::Result<String> {
    BundleProfile::Release.signing()?;
    required(PROFILE_VARIABLE)
}

pub(crate) fn publish(root: &Path, app: &Path, version: &str) -> io::Result<Published> {
    let keychain_profile = require_credentials()?;
    let signing = BundleProfile::Release.signing()?;
    let directory = root.join(DISTRIBUTION_DIRECTORY);
    fs::create_dir_all(&directory)?;
    let zip = directory.join(format!("DOMOVOI-{version}.zip"));
    let disk_image = directory.join(format!("DOMOVOI-{version}.dmg"));
    run(Command::new(DITTO)
        .args(["-c", "-k", "--keepParent"])
        .arg(app)
        .arg(&zip))?;
    notarize(&zip, &keychain_profile)?;
    staple(app)?;
    run(Command::new(DITTO)
        .args(["-c", "-k", "--keepParent"])
        .arg(app)
        .arg(&zip))?;
    run(Command::new(HDIUTIL)
        .args([
            "create",
            "-ov",
            "-format",
            "UDZO",
            "-volname",
            VOLUME_NAME,
            "-srcfolder",
        ])
        .arg(app)
        .arg(&disk_image))?;
    signing.sign(&disk_image, None)?;
    notarize(&disk_image, &keychain_profile)?;
    staple(&disk_image)?;
    Ok(Published { zip, disk_image })
}

fn notarize(artifact: &Path, keychain_profile: &str) -> io::Result<()> {
    let output = Command::new(XCRUN)
        .args(["notarytool", "submit"])
        .arg(artifact)
        .args(["--keychain-profile", keychain_profile, "--wait"])
        .output()?;
    let report = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && report.contains(ACCEPTED) {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "notarization of {} was not accepted:\n{report}{}",
            artifact.display(),
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn staple(artifact: &Path) -> io::Result<()> {
    run(Command::new(XCRUN).args(["stapler", "staple"]).arg(artifact))
}

fn run(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{command:?} failed with {status}")))
    }
}
