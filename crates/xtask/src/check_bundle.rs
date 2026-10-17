use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value};

use crate::bundle_layout::{
    APP_EXECUTABLE, BundleLayout, CATALOG, DIAGRAM_METALLIB, SERVICE_EXECUTABLE, SERVICE_METALLIBS,
};
use crate::finding::Finding;
use crate::property_list;
use crate::workspace;
use crate::workspace_version;

const CODESIGN: &str = "/usr/bin/codesign";
const LIPO: &str = "/usr/bin/lipo";
const OTOOL: &str = "/usr/bin/otool";
const ARM64_ONLY: &str = "is architecture: arm64";
const FRAMEWORKS_RPATH: &str = "path @executable_path/../Frameworks ";
const MINIMUM_SYSTEM: &str = "15.0";
const APP_IDENTIFIER: &str = "com.domovoi.app";
const SERVICE_IDENTIFIER: &str = "com.domovoi.inference";
const TEAM_KEY: &str = "DOMOVOITeamIdentifier";
const SANDBOX: &str = "com.apple.security.app-sandbox";
const NETWORK_PREFIX: &str = "com.apple.security.network.";
const FORBIDDEN_ENTITLEMENTS: [&str; 2] = [
    "com.apple.security.cs.allow-jit",
    "com.apple.security.cs.disable-library-validation",
];
const USAGE_DESCRIPTIONS: [&str; 3] = [
    "NSDesktopFolderUsageDescription",
    "NSDocumentsFolderUsageDescription",
    "NSDownloadsFolderUsageDescription",
];
const ED25519_KEY_LENGTH: usize = 44;
const FONTS: &str = "assets/fonts";
const AD_HOC_MARKER: &str = "Signature=adhoc";
const TEAM_MARKER: &str = "TeamIdentifier=";
const RUNTIME_MARKER: &str = "(runtime)";

pub(crate) fn run(app: &str) -> io::Result<Vec<Finding>> {
    let root = workspace::root();
    let layout = BundleLayout::of_app(Path::new(app));
    let version = workspace_version::read(&root)?;
    let mut findings = structure(&root, &layout)?;
    if !findings.is_empty() {
        return Ok(findings);
    }
    for executable in [layout.executable(), layout.service_executable()] {
        let output = tool(LIPO, &["-info"], &executable)?;
        if !output.trim_end().ends_with(ARM64_ONLY) {
            findings.push(finding(&executable, format!("not arm64-only: {}", output.trim())));
        }
    }
    if !tool(OTOOL, &["-l"], &layout.executable())?.contains(FRAMEWORKS_RPATH) {
        findings.push(finding(
            &layout.executable(),
            "no rpath @executable_path/../Frameworks".to_owned(),
        ));
    }
    let app_info = property_list::read(&layout.info())?;
    findings.extend(app_keys(&layout.info(), &app_info, &version));
    let service_info = property_list::read(&layout.service_info())?;
    findings.extend(service_keys(&layout.service_info(), &service_info, &version));
    findings.extend(signatures(&layout, &service_info)?);
    Ok(findings)
}

fn structure(root: &Path, layout: &BundleLayout) -> io::Result<Vec<Finding>> {
    let mut required: Vec<PathBuf> = vec![
        layout.executable(),
        layout.info(),
        layout.resources().join(CATALOG),
        layout.resources().join(DIAGRAM_METALLIB),
        layout.licenses().join("Sparkle-LICENSE.txt"),
        layout.licenses().join("llama.cpp-LICENSE.txt"),
        layout.service_executable(),
        layout.service_info(),
        layout.sparkle().join("Versions/B/Sparkle"),
        layout.sparkle().join("Versions/B/Autoupdate"),
        layout.sparkle().join("Versions/B/Updater.app"),
    ];
    required.extend(
        SERVICE_METALLIBS
            .iter()
            .map(|metallib| layout.service_resources().join(metallib)),
    );
    for family in fs::read_dir(root.join(FONTS))? {
        let family = family?.path();
        if !family.is_dir() {
            continue;
        }
        let name = family
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        required.push(layout.licenses().join(format!("{name}-OFL.txt")));
        for file in fs::read_dir(&family)? {
            let file = file?.path();
            if file.extension().is_some_and(|extension| extension == "ttf") {
                required.push(layout.fonts().join(file.file_name().expect("a font file has a name")));
            }
        }
    }
    let mut findings: Vec<Finding> = required
        .into_iter()
        .filter(|path| !path.exists())
        .map(|path| finding(&path, "is missing".to_owned()))
        .collect();
    for forbidden in [
        layout.sparkle().join("Versions/B/XPCServices"),
        layout.sparkle().join("XPCServices"),
    ] {
        if fs::symlink_metadata(&forbidden).is_ok() {
            findings.push(finding(
                &forbidden,
                "the Sparkle sandbox services must not ship".to_owned(),
            ));
        }
    }
    Ok(findings)
}

fn app_keys(path: &Path, info: &Value, version: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (key, expected) in [
        ("CFBundleIdentifier", APP_IDENTIFIER),
        ("CFBundleExecutable", APP_EXECUTABLE),
        ("CFBundlePackageType", "APPL"),
        ("LSMinimumSystemVersion", MINIMUM_SYSTEM),
        ("ATSApplicationFontsPath", "Fonts"),
        ("CFBundleShortVersionString", version),
        ("CFBundleVersion", version),
    ] {
        expect_string(path, info, key, expected, &mut findings);
    }
    let feed = info["SUFeedURL"].as_str().unwrap_or_default();
    let is_xml = Path::new(feed)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"));
    if !(feed.starts_with("https://") && is_xml) {
        findings.push(finding(path, format!("SUFeedURL {feed:?} is not an https appcast")));
    }
    if !is_ed25519_public_key(info["SUPublicEDKey"].as_str().unwrap_or_default()) {
        findings.push(finding(
            path,
            "SUPublicEDKey is not a base64 Ed25519 public key".to_owned(),
        ));
    }
    for key in USAGE_DESCRIPTIONS {
        if info[key].as_str().is_none_or(|text| text.trim().is_empty()) {
            findings.push(finding(path, format!("{key} is missing")));
        }
    }
    findings
}

fn service_keys(path: &Path, info: &Value, version: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (key, expected) in [
        ("CFBundleIdentifier", SERVICE_IDENTIFIER),
        ("CFBundleExecutable", SERVICE_EXECUTABLE),
        ("CFBundlePackageType", "XPC!"),
        ("LSMinimumSystemVersion", MINIMUM_SYSTEM),
        ("CFBundleShortVersionString", version),
        ("CFBundleVersion", version),
    ] {
        expect_string(path, info, key, expected, &mut findings);
    }
    if info["XPCService"]["ServiceType"].as_str() != Some("Application") {
        findings.push(finding(path, "XPCService.ServiceType is not Application".to_owned()));
    }
    findings
}

fn signatures(layout: &BundleLayout, service_info: &Value) -> io::Result<Vec<Finding>> {
    let mut findings = Vec::new();
    let verified = Command::new(CODESIGN)
        .args(["--verify", "--deep", "--strict"])
        .arg(&layout.app)
        .output()?;
    if !verified.status.success() {
        findings.push(finding(
            &layout.app,
            format!(
                "codesign --verify failed: {}",
                String::from_utf8_lossy(&verified.stderr).trim()
            ),
        ));
    }
    let app = describe(&layout.app)?;
    let service = describe(&layout.service())?;
    let ad_hoc = app.contains(AD_HOC_MARKER);
    if ad_hoc != service.contains(AD_HOC_MARKER) {
        findings.push(finding(
            &layout.service(),
            "the service and the application are signed differently".to_owned(),
        ));
    }
    let declared_team = service_info[TEAM_KEY].as_str();
    if ad_hoc {
        if declared_team.is_some() {
            findings.push(finding(
                &layout.service_info(),
                format!("an ad-hoc service carries {TEAM_KEY}"),
            ));
        }
    } else {
        let signed_team = team_of(&service);
        if declared_team.is_none() || declared_team != signed_team.as_deref() || team_of(&app) != signed_team {
            findings.push(finding(
                &layout.service_info(),
                format!("{TEAM_KEY} {declared_team:?} differs from the signing team {signed_team:?}"),
            ));
        }
        for (path, description) in [(&layout.app, &app), (&layout.service(), &service)] {
            if !description.contains(RUNTIME_MARKER) {
                findings.push(finding(path, "is signed without the hardened runtime".to_owned()));
            }
        }
    }
    let service_entitlements = entitlements(&layout.service())?;
    if service_entitlements.get(SANDBOX) != Some(&Value::Bool(true)) {
        findings.push(finding(&layout.service(), format!("{SANDBOX} is not granted")));
    }
    for (path, granted) in [
        (&layout.app, entitlements(&layout.app)?),
        (&layout.service(), service_entitlements),
    ] {
        for key in granted.keys() {
            if key.starts_with(NETWORK_PREFIX) || FORBIDDEN_ENTITLEMENTS.contains(&key.as_str()) {
                findings.push(finding(path, format!("{key} must not be requested")));
            }
        }
    }
    Ok(findings)
}

fn describe(path: &Path) -> io::Result<String> {
    let output = Command::new(CODESIGN).arg("-dvvv").arg(path).output()?;
    Ok(String::from_utf8_lossy(&output.stderr).into_owned())
}

fn team_of(description: &str) -> Option<String> {
    description
        .lines()
        .find_map(|line| line.strip_prefix(TEAM_MARKER))
        .map(str::to_owned)
}

fn entitlements(path: &Path) -> io::Result<Map<String, Value>> {
    let output = Command::new(CODESIGN)
        .args(["-d", "--entitlements", "-", "--xml"])
        .arg(path)
        .output()?;
    if output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Map::new());
    }
    match property_list::parse_xml(&output.stdout, &path.display().to_string())? {
        Value::Object(map) => Ok(map),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "entitlements are not a dictionary",
        )),
    }
}

fn tool(program: &str, arguments: &[&str], path: &Path) -> io::Result<String> {
    let output = Command::new(program).args(arguments).arg(path).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!("{program} {} failed", path.display())));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn expect_string(path: &Path, info: &Value, key: &str, expected: &str, findings: &mut Vec<Finding>) {
    let actual = info[key].as_str();
    if actual != Some(expected) {
        findings.push(finding(path, format!("{key} is {actual:?}, expected {expected:?}")));
    }
}

fn is_ed25519_public_key(text: &str) -> bool {
    text.len() == ED25519_KEY_LENGTH
        && text.ends_with('=')
        && !text.ends_with("==")
        && text[..ED25519_KEY_LENGTH - 1]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/')
}

fn finding(path: &Path, reason: String) -> Finding {
    Finding {
        path: path.to_path_buf(),
        line: 0,
        reason,
    }
}
