use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::git::git;

const CHANGELOG: &str = "CHANGELOG.md";
const HEADER: &str = "# Changelog";
const DATE: &str = "/bin/date";

pub(crate) fn record(root: &Path, version: &str) -> io::Result<()> {
    let previous = git(root, &["describe", "--tags", "--abbrev=0"]).ok();
    let range = previous.map(|tag| format!("{}..HEAD", tag.trim()));
    let mut arguments = vec!["log", "--format=%s"];
    if let Some(range) = &range {
        arguments.push(range);
    }
    let changes: Vec<String> = git(root, &arguments)?
        .lines()
        .filter(|subject| !subject.trim().is_empty())
        .map(|subject| format!("- {subject}"))
        .collect();
    let date = Command::new(DATE).arg("+%F").output()?;
    let entry = format!(
        "## {version} — {}\n\n{}\n",
        String::from_utf8_lossy(&date.stdout).trim(),
        changes.join("\n")
    );
    let path = root.join(CHANGELOG);
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => format!("{HEADER}\n"),
        Err(error) => return Err(error),
    };
    let body = existing
        .strip_prefix(HEADER)
        .unwrap_or(&existing)
        .trim_start_matches('\n');
    fs::write(path, format!("{HEADER}\n\n{entry}\n{body}"))
}
