use std::fs;
use std::io;
use std::path::Path;

const MANIFEST: &str = "Cargo.toml";
const SECTION: &str = "[workspace.package]";
const VERSION_PREFIX: &str = "version = \"";

pub(crate) fn read(root: &Path) -> io::Result<String> {
    let text = fs::read_to_string(root.join(MANIFEST))?;
    version_line(&text).map(|(_, version)| version)
}

pub(crate) fn bump_patch(root: &Path) -> io::Result<String> {
    let manifest = root.join(MANIFEST);
    let text = fs::read_to_string(&manifest)?;
    let (line, version) = version_line(&text)?;
    let parts: Vec<u64> = version
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| invalid(&format!("the workspace version {version} is not numeric")))?;
    let [major, minor, patch] = parts.as_slice() else {
        return Err(invalid(&format!(
            "the workspace version {version} is not major.minor.patch"
        )));
    };
    let bumped = format!("{major}.{minor}.{}", patch + 1);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines[line] = format!("{VERSION_PREFIX}{bumped}\"");
    fs::write(&manifest, lines.join("\n") + "\n")?;
    Ok(bumped)
}

fn version_line(text: &str) -> io::Result<(usize, String)> {
    let section = text
        .lines()
        .position(|line| line.trim() == SECTION)
        .ok_or_else(|| invalid("the workspace manifest has no [workspace.package]"))?;
    text.lines()
        .enumerate()
        .skip(section + 1)
        .take_while(|(_, line)| !line.starts_with('['))
        .find_map(|(index, line)| {
            line.strip_prefix(VERSION_PREFIX)
                .and_then(|rest| rest.strip_suffix('"'))
                .map(|version| (index, version.to_owned()))
        })
        .ok_or_else(|| invalid("[workspace.package] has no version"))
}

fn invalid(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason.to_owned())
}
