use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SCANNED_EXTENSIONS: [&str; 2] = ["rs", "metal"];

pub(crate) fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask lives two levels below the workspace root")
        .to_path_buf()
}

pub(crate) fn crates_dir() -> PathBuf {
    root().join("crates")
}

pub(crate) fn source_files(directory: &Path, excluded_directories: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_source_files(directory, excluded_directories, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_source_files(
    directory: &Path,
    excluded_directories: &[PathBuf],
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            if is_walkable_directory(&path, excluded_directories) {
                collect_source_files(&path, excluded_directories, files)?;
            }
        } else if has_scanned_extension(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_walkable_directory(path: &Path, excluded_directories: &[PathBuf]) -> bool {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    !name.starts_with('.') && name != "target" && !excluded_directories.iter().any(|excluded| excluded == path)
}

fn has_scanned_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SCANNED_EXTENSIONS.contains(&extension))
}
