use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

const TEMPORARY_SUFFIX: &str = "partial";

pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file path has no directory"))?;
    fs::create_dir_all(directory)?;
    let temporary = path.with_extension(format!("{TEMPORARY_SUFFIX}-{}", std::process::id()));
    let mut output = File::create(&temporary)?;
    output.write_all(bytes)?;
    output.sync_all()?;
    fs::rename(&temporary, path)
}
