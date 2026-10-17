use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::property_list;
use crate::sparkle_distribution::SparkleDistribution;

const APP_TEMPLATE: &str = "crates/app/bundle/Info.plist";
const FEED_KEY: &str = "SUFeedURL";
const LATEST_DOWNLOAD: &str = "/releases/latest/download/";
const ARCHIVES: &str = "dist/appcast";
const APPCAST: &str = "appcast.xml";
const GENERATOR: &str = "generate_appcast";
const MINIMUM_SYSTEM: &str = "<sparkle:minimumSystemVersion>15.0</sparkle:minimumSystemVersion>";

pub(crate) fn generate(
    root: &Path,
    sparkle: &SparkleDistribution,
    archive: &Path,
    version: &str,
) -> io::Result<PathBuf> {
    let info = property_list::read(&root.join(APP_TEMPLATE))?;
    let feed = info[FEED_KEY]
        .as_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("{APP_TEMPLATE} has no {FEED_KEY}")))?;
    let (releases, _) = feed.split_once(LATEST_DOWNLOAD).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{FEED_KEY} {feed} is not a GitHub latest-release download"),
        )
    })?;
    let prefix = format!("{releases}/releases/download/v{version}/");
    let archives = root.join(ARCHIVES);
    fs::create_dir_all(&archives)?;
    let name = archive
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the archive has no file name"))?;
    fs::copy(archive, archives.join(name))?;
    let appcast = archives.join(APPCAST);
    let status = Command::new(sparkle.tool(GENERATOR))
        .args(["--download-url-prefix", &prefix, "-o"])
        .arg(&appcast)
        .arg(&archives)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!("{GENERATOR} failed with {status}")));
    }
    let text = fs::read_to_string(&appcast)?;
    if !text.contains(MINIMUM_SYSTEM) || !text.contains(&format!(">{version}<")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} lacks version {version} or the minimum system 15.0",
                appcast.display()
            ),
        ));
    }
    Ok(appcast)
}
