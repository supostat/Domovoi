use std::path::{Path, PathBuf};

const HOME: &str = "~";
const HOME_PREFIX: &str = "~/";

pub(crate) fn expanded(path: &str, home: &Path) -> PathBuf {
    if path == HOME {
        return home.to_path_buf();
    }
    match path.strip_prefix(HOME_PREFIX) {
        Some(rest) => home.join(rest),
        None => PathBuf::from(path),
    }
}
