#![allow(dead_code)]

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_scanner::{ScanOptions, scan};

const CODESIGN: &str = "/usr/bin/codesign";
const EXECUTABLE_SOURCE: &str = "/usr/bin/true";
const CONTAINER_METADATA: &str = ".com.apple.containermanagerd.metadata.plist";

pub(crate) enum Value<'text> {
    Text(&'text str),
    Texts(&'text [&'text str]),
}

pub(crate) fn property_list(entries: &[(&str, Value<'_>)]) -> String {
    let mut body = String::new();
    for (key, value) in entries {
        write!(body, "<key>{key}</key>").expect("a string takes the key");
        match value {
            Value::Text(text) => write!(body, "<string>{text}</string>").expect("a string takes the value"),
            Value::Texts(texts) => {
                body.push_str("<array>");
                for text in *texts {
                    write!(body, "<string>{text}</string>").expect("a string takes the item");
                }
                body.push_str("</array>");
            }
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>{body}</dict></plist>\n"
    )
}

pub(crate) fn write_property_list(path: &Path, entries: &[(&str, Value<'_>)]) {
    fs::create_dir_all(path.parent().expect("a property list has a folder")).expect("the folder is created");
    fs::write(path, property_list(entries)).expect("the property list is written");
}

pub(crate) fn application(folder: &Path, name: &str, bundle_id: &str, app_groups: &[&str]) -> PathBuf {
    let bundle = folder.join(format!("{name}.app"));
    let executable = bundle.join("Contents/MacOS").join(name);
    fs::create_dir_all(executable.parent().expect("the executable has a folder")).expect("the bundle is created");
    fs::copy(EXECUTABLE_SOURCE, &executable).expect("the executable is copied");
    write_property_list(
        &bundle.join("Contents/Info.plist"),
        &[
            ("CFBundleIdentifier", Value::Text(bundle_id)),
            ("CFBundleName", Value::Text(name)),
            ("CFBundleExecutable", Value::Text(name)),
            ("CFBundlePackageType", Value::Text("APPL")),
        ],
    );
    let entitlements = folder.join(format!(".{name}.entitlements"));
    write_property_list(
        &entitlements,
        &[("com.apple.security.application-groups", Value::Texts(app_groups))],
    );
    let status = Command::new(CODESIGN)
        .args(["--force", "--sign", "-", "--entitlements"])
        .arg(&entitlements)
        .arg(&bundle)
        .status()
        .expect("codesign runs");
    assert!(status.success(), "codesign failed for {}", bundle.display());
    fs::remove_file(&entitlements).expect("the entitlements file is removed");
    bundle
}

pub(crate) fn container(folder: &Path, identifier: &str) -> PathBuf {
    write_property_list(
        &folder.join(CONTAINER_METADATA),
        &[("MCMMetadataIdentifier", Value::Text(identifier))],
    );
    fs::write(folder.join("data.db"), vec![1_u8; 4_096]).expect("container data is written");
    folder.to_path_buf()
}

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
