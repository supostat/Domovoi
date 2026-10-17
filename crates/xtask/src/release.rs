use std::io;

use crate::appcast;
use crate::bundle::assemble;
use crate::bundle_profile::BundleProfile;
use crate::changelog;
use crate::finding::Finding;
use crate::git::git;
use crate::notarization::{publish, require_credentials};
use crate::sparkle_distribution::SparkleDistribution;
use crate::workspace;
use crate::workspace_version;

const RELEASED_FILES: [&str; 3] = ["Cargo.toml", "Cargo.lock", "CHANGELOG.md"];

pub(crate) fn run() -> io::Result<Vec<Finding>> {
    let root = workspace::root();
    if !git(&root, &["status", "--porcelain"])?.trim().is_empty() {
        return Err(io::Error::other("the working tree has uncommitted changes"));
    }
    require_credentials()?;
    let sparkle = SparkleDistribution::ensure(&root)?;
    let version = workspace_version::bump_patch(&root)?;
    let app = assemble(&root, BundleProfile::Release, &version)?;
    let published = publish(&root, &app, &version)?;
    let appcast = appcast::generate(&root, &sparkle, &published.zip, &version)?;
    changelog::record(&root, &version)?;
    let mut add = vec!["add", "--"];
    add.extend(RELEASED_FILES);
    git(&root, &add)?;
    let message = format!("Release {version}\n\nBump the workspace version, record the changelog\nand tag v{version}.");
    git(&root, &["commit", "--quiet", "--message", &message])?;
    git(
        &root,
        &[
            "tag",
            "--annotate",
            &format!("v{version}"),
            "--message",
            &format!("DOMOVOI {version}"),
        ],
    )?;
    println!("v{version} is tagged; upload to the GitHub release:");
    for artifact in [&published.zip, &published.disk_image, &appcast] {
        println!("  {}", artifact.display());
    }
    Ok(Vec::new())
}
