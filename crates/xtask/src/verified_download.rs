use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

const CURL: &str = "/usr/bin/curl";
const SHASUM: &str = "/usr/bin/shasum";
const PARTIAL_EXTENSION: &str = "partial";

pub(crate) fn is_verified(path: &Path, size: u64, sha256: &str) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len() == size && digest(path)? == sha256),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn download_verified(url: &str, path: &Path, size: u64, sha256: &str) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a download target has no directory"))?;
    fs::create_dir_all(directory)?;
    let partial = path.with_extension(PARTIAL_EXTENSION);
    let status = Command::new(CURL)
        .args(["--fail", "--location", "--silent", "--show-error", "--output"])
        .arg(&partial)
        .arg(url)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!("downloading {url} failed with {status}")));
    }
    let received_size = fs::metadata(&partial)?.len();
    let received = digest(&partial)?;
    if received_size != size || received != sha256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{url} gave {received_size} bytes with SHA-256 {received}, expected {size} bytes with {sha256}"),
        ));
    }
    fs::rename(partial, path)
}

fn digest(path: &Path) -> io::Result<String> {
    let output = Command::new(SHASUM).args(["-a", "256"]).arg(path).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!("shasum failed for {}", path.display())));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "shasum printed non-UTF-8 output"))?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "shasum printed no digest"))
}
