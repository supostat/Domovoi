use std::path::Path;

const HOME: &str = "~";

pub(crate) fn tilde(path: &Path) -> String {
    match std::env::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(relative) if relative.as_os_str().is_empty() => HOME.to_owned(),
        Some(relative) => format!("{HOME}/{}", relative.display()),
        None => path.display().to_string(),
    }
}
