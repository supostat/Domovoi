use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

const PLUTIL: &str = "/usr/bin/plutil";

pub(crate) fn read(path: &Path) -> io::Result<Value> {
    let output = Command::new(PLUTIL)
        .args(["-convert", "json", "-o", "-"])
        .arg(path)
        .output()?;
    parse(&output.stdout, output.status.success(), &path.display().to_string())
}

pub(crate) fn parse_xml(xml: &[u8], origin: &str) -> io::Result<Value> {
    let mut child = Command::new(PLUTIL)
        .args(["-convert", "json", "-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("plutil has no input"))?
        .write_all(xml)?;
    let output = child.wait_with_output()?;
    parse(&output.stdout, output.status.success(), origin)
}

pub(crate) fn set_string(path: &Path, key: &str, value: &str) -> io::Result<()> {
    edit(path, &["-replace", key, "-string", value])
}

pub(crate) fn remove(path: &Path, key: &str) -> io::Result<()> {
    edit(path, &["-remove", key])
}

fn edit(path: &Path, arguments: &[&str]) -> io::Result<()> {
    let status = Command::new(PLUTIL).args(arguments).arg(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "plutil {arguments:?} {} failed",
            path.display()
        )))
    }
}

fn parse(json: &[u8], converted: bool, origin: &str) -> io::Result<Value> {
    if !converted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{origin} is not a property list"),
        ));
    }
    serde_json::from_slice(json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{origin}: {error}")))
}
