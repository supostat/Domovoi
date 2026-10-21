use std::path::{Path, PathBuf};

use domovoi_index::Index;

const HOME: &str = "~";
const HOME_PREFIX: &str = "~/";

pub(crate) fn resolved(path: &str, home: &Path, index: &Index) -> PathBuf {
    let literal = expanded(path, home);
    if index.find(&literal).is_some() {
        return literal;
    }
    home_name_dropped(path, home)
        .filter(|candidate| index.find(candidate).is_some())
        .unwrap_or(literal)
}

fn expanded(path: &str, home: &Path) -> PathBuf {
    if path == HOME {
        return home.to_path_buf();
    }
    match path.strip_prefix(HOME_PREFIX) {
        Some(rest) => home.join(rest),
        None => PathBuf::from(path),
    }
}

fn home_name_dropped(path: &str, home: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(HOME_PREFIX)?;
    let name = home.file_name()?.to_str()?;
    if rest == name {
        return Some(home.to_path_buf());
    }
    Some(home.join(rest.strip_prefix(name)?.strip_prefix('/')?))
}
