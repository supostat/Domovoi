use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::property_list::{PropertyValue, write_property_list};

const CODESIGN: &str = "/usr/bin/codesign";
const EXECUTABLE_SOURCE: &str = "/usr/bin/true";
const CONTAINER_METADATA: &str = ".com.apple.containermanagerd.metadata.plist";
const CONTAINER_DATA: &str = "data.db";
const CONTAINER_DATA_BYTES: usize = 4_096;

pub fn application_bundle(folder: &Path, name: &str, bundle_id: &str, app_groups: &[&str]) -> io::Result<PathBuf> {
    let bundle = folder.join(format!("{name}.app"));
    let executable = bundle.join("Contents/MacOS").join(name);
    fs::create_dir_all(bundle.join("Contents/MacOS"))?;
    fs::copy(EXECUTABLE_SOURCE, &executable)?;
    write_property_list(
        &bundle.join("Contents/Info.plist"),
        &[
            ("CFBundleIdentifier", PropertyValue::Text(bundle_id)),
            ("CFBundleName", PropertyValue::Text(name)),
            ("CFBundleExecutable", PropertyValue::Text(name)),
            ("CFBundlePackageType", PropertyValue::Text("APPL")),
        ],
    )?;
    let entitlements = folder.join(format!(".{name}.entitlements"));
    write_property_list(
        &entitlements,
        &[(
            "com.apple.security.application-groups",
            PropertyValue::Texts(app_groups),
        )],
    )?;
    let signed = Command::new(CODESIGN)
        .args(["--force", "--sign", "-", "--entitlements"])
        .arg(&entitlements)
        .arg(&bundle)
        .output()?;
    fs::remove_file(&entitlements)?;
    if !signed.status.success() {
        return Err(io::Error::other(format!(
            "codesign failed for {}: {}",
            bundle.display(),
            String::from_utf8_lossy(&signed.stderr)
        )));
    }
    Ok(bundle)
}

pub fn app_container(folder: &Path, identifier: &str) -> io::Result<PathBuf> {
    write_property_list(
        &folder.join(CONTAINER_METADATA),
        &[("MCMMetadataIdentifier", PropertyValue::Text(identifier))],
    )?;
    fs::write(folder.join(CONTAINER_DATA), vec![1_u8; CONTAINER_DATA_BYTES])?;
    Ok(folder.to_path_buf())
}
